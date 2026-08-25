// SPDX-License-Identifier: Apache-2.0

using System;
using System.Collections.Generic;
using System.Globalization;
using System.Text;

namespace IdleWarden.Bridge
{
    public enum JsonKind
    {
        Null,
        Bool,
        Integer,
        Number,
        String,
        Array,
        Object,
    }

    /// <summary>
    /// A JSON tree, hand-rolled on purpose.
    /// <para>
    /// A mod is loaded into a game process whose assembly versions we do not
    /// control. System.Text.Json and Newtonsoft both drag dependencies that
    /// routinely collide with what a Unity game already ships, and a collision
    /// here breaks the game, not just the mod. The protocol is small and fully
    /// specified, so carrying our own reader costs less than that risk.
    /// </para>
    /// </summary>
    public sealed class JsonValue
    {
        private readonly bool boolean;
        private readonly long integer;
        private readonly double number;
        private readonly string text;
        private readonly List<JsonValue> array;
        private readonly Dictionary<string, JsonValue> members;

        public JsonKind Kind { get; }

        private JsonValue(
            JsonKind kind,
            bool boolean = false,
            long integer = 0,
            double number = 0,
            string text = null,
            List<JsonValue> array = null,
            Dictionary<string, JsonValue> members = null)
        {
            Kind = kind;
            this.boolean = boolean;
            this.integer = integer;
            this.number = number;
            this.text = text;
            this.array = array;
            this.members = members;
        }

        public static readonly JsonValue Null = new JsonValue(JsonKind.Null);

        public static JsonValue Bool(bool value) => new JsonValue(JsonKind.Bool, boolean: value);

        public static JsonValue Integer(long value) =>
            new JsonValue(JsonKind.Integer, integer: value);

        public static JsonValue Number(double value)
        {
            if (double.IsNaN(value) || double.IsInfinity(value))
            {
                throw new ArgumentOutOfRangeException(nameof(value), "JSON has no NaN or infinity");
            }

            return new JsonValue(JsonKind.Number, number: value);
        }

        public static JsonValue String(string value) =>
            new JsonValue(JsonKind.String, text: value ?? throw new ArgumentNullException(nameof(value)));

        public static JsonValue Array(List<JsonValue> values) =>
            new JsonValue(JsonKind.Array, array: values ?? new List<JsonValue>());

        public static JsonValue Object(Dictionary<string, JsonValue> values) =>
            new JsonValue(JsonKind.Object, members: values ?? new Dictionary<string, JsonValue>());

        public bool AsBool() => Kind == JsonKind.Bool
            ? boolean
            : throw new JsonException("expected a boolean, got " + Kind);

        public long AsInteger() => Kind == JsonKind.Integer
            ? integer
            : throw new JsonException("expected an integer, got " + Kind);

        public double AsNumber()
        {
            if (Kind == JsonKind.Number)
            {
                return number;
            }

            if (Kind == JsonKind.Integer)
            {
                return integer;
            }

            throw new JsonException("expected a number, got " + Kind);
        }

        public string AsString() => Kind == JsonKind.String
            ? text
            : throw new JsonException("expected a string, got " + Kind);

        public IReadOnlyList<JsonValue> AsArray() => Kind == JsonKind.Array
            ? array
            : throw new JsonException("expected an array, got " + Kind);

        public IReadOnlyDictionary<string, JsonValue> AsObject() => Kind == JsonKind.Object
            ? members
            : throw new JsonException("expected an object, got " + Kind);

        public JsonValue Member(string name)
        {
            if (Kind != JsonKind.Object)
            {
                throw new JsonException("expected an object, got " + Kind);
            }

            return members.TryGetValue(name, out var found)
                ? found
                : throw new JsonException("missing field `" + name + "`");
        }

        public JsonValue MemberOrNull(string name)
        {
            if (Kind != JsonKind.Object)
            {
                throw new JsonException("expected an object, got " + Kind);
            }

            return members.TryGetValue(name, out var found) ? found : null;
        }

        public override string ToString()
        {
            var builder = new StringBuilder();
            Write(builder, this);
            return builder.ToString();
        }

        private static void Write(StringBuilder builder, JsonValue value)
        {
            switch (value.Kind)
            {
                case JsonKind.Null:
                    builder.Append("null");
                    break;
                case JsonKind.Bool:
                    builder.Append(value.boolean ? "true" : "false");
                    break;
                case JsonKind.Integer:
                    builder.Append(value.integer.ToString(CultureInfo.InvariantCulture));
                    break;
                case JsonKind.Number:
                    builder.Append(value.number.ToString("R", CultureInfo.InvariantCulture));
                    break;
                case JsonKind.String:
                    WriteString(builder, value.text);
                    break;
                case JsonKind.Array:
                    WriteArray(builder, value.array);
                    break;
                default:
                    WriteObject(builder, value.members);
                    break;
            }
        }

        private static void WriteArray(StringBuilder builder, List<JsonValue> values)
        {
            builder.Append('[');
            for (var i = 0; i < values.Count; i++)
            {
                if (i > 0)
                {
                    builder.Append(',');
                }

                Write(builder, values[i]);
            }

            builder.Append(']');
        }

        private static void WriteObject(StringBuilder builder, Dictionary<string, JsonValue> values)
        {
            builder.Append('{');
            var first = true;
            foreach (var pair in values)
            {
                if (!first)
                {
                    builder.Append(',');
                }

                first = false;
                WriteString(builder, pair.Key);
                builder.Append(':');
                Write(builder, pair.Value);
            }

            builder.Append('}');
        }

        private static void WriteString(StringBuilder builder, string value)
        {
            builder.Append('"');
            foreach (var c in value)
            {
                switch (c)
                {
                    case '"':
                        builder.Append("\\\"");
                        break;
                    case '\\':
                        builder.Append("\\\\");
                        break;
                    case '\n':
                        builder.Append("\\n");
                        break;
                    case '\r':
                        builder.Append("\\r");
                        break;
                    case '\t':
                        builder.Append("\\t");
                        break;
                    default:
                        if (c < ' ')
                        {
                            builder.Append("\\u").Append(((int)c).ToString("x4", CultureInfo.InvariantCulture));
                        }
                        else
                        {
                            builder.Append(c);
                        }

                        break;
                }
            }

            builder.Append('"');
        }

        public static JsonValue Parse(string text)
        {
            if (text == null)
            {
                throw new ArgumentNullException(nameof(text));
            }

            var reader = new JsonReader(text);
            var value = reader.ReadValue();
            reader.SkipWhitespace();
            if (!reader.AtEnd)
            {
                throw new JsonException("trailing content after the JSON value");
            }

            return value;
        }
    }

    public sealed class JsonException : Exception
    {
        public JsonException(string message) : base(message)
        {
        }
    }

    internal sealed class JsonReader
    {
        private const int MaxDepth = 32;

        private readonly string text;
        private int at;
        private int depth;

        internal JsonReader(string text)
        {
            this.text = text;
        }

        internal bool AtEnd => at >= text.Length;

        internal void SkipWhitespace()
        {
            while (at < text.Length && (text[at] == ' ' || text[at] == '\t' || text[at] == '\n' || text[at] == '\r'))
            {
                at++;
            }
        }

        internal JsonValue ReadValue()
        {
            if (++depth > MaxDepth)
            {
                throw new JsonException("nesting is deeper than " + MaxDepth);
            }

            try
            {
                SkipWhitespace();
                if (AtEnd)
                {
                    throw new JsonException("unexpected end of input");
                }

                switch (text[at])
                {
                    case '{':
                        return ReadObject();
                    case '[':
                        return ReadArray();
                    case '"':
                        return JsonValue.String(ReadString());
                    case 't':
                        Expect("true");
                        return JsonValue.Bool(true);
                    case 'f':
                        Expect("false");
                        return JsonValue.Bool(false);
                    case 'n':
                        Expect("null");
                        return JsonValue.Null;
                    default:
                        return ReadNumber();
                }
            }
            finally
            {
                depth--;
            }
        }

        private void Expect(string literal)
        {
            if (at + literal.Length > text.Length || string.CompareOrdinal(text, at, literal, 0, literal.Length) != 0)
            {
                throw new JsonException("expected `" + literal + "`");
            }

            at += literal.Length;
        }

        private JsonValue ReadObject()
        {
            at++;
            var members = new Dictionary<string, JsonValue>();
            SkipWhitespace();
            if (!AtEnd && text[at] == '}')
            {
                at++;
                return JsonValue.Object(members);
            }

            while (true)
            {
                SkipWhitespace();
                var name = ReadString();
                SkipWhitespace();
                if (AtEnd || text[at] != ':')
                {
                    throw new JsonException("expected `:` after a field name");
                }

                at++;
                members[name] = ReadValue();
                SkipWhitespace();
                if (AtEnd)
                {
                    throw new JsonException("unterminated object");
                }

                if (text[at] == ',')
                {
                    at++;
                    continue;
                }

                if (text[at] == '}')
                {
                    at++;
                    return JsonValue.Object(members);
                }

                throw new JsonException("expected `,` or `}` in an object");
            }
        }

        private JsonValue ReadArray()
        {
            at++;
            var values = new List<JsonValue>();
            SkipWhitespace();
            if (!AtEnd && text[at] == ']')
            {
                at++;
                return JsonValue.Array(values);
            }

            while (true)
            {
                values.Add(ReadValue());
                SkipWhitespace();
                if (AtEnd)
                {
                    throw new JsonException("unterminated array");
                }

                if (text[at] == ',')
                {
                    at++;
                    continue;
                }

                if (text[at] == ']')
                {
                    at++;
                    return JsonValue.Array(values);
                }

                throw new JsonException("expected `,` or `]` in an array");
            }
        }

        private string ReadString()
        {
            if (AtEnd || text[at] != '"')
            {
                throw new JsonException("expected a string");
            }

            at++;
            var builder = new StringBuilder();
            while (true)
            {
                if (AtEnd)
                {
                    throw new JsonException("unterminated string");
                }

                var c = text[at++];
                if (c == '"')
                {
                    return builder.ToString();
                }

                if (c != '\\')
                {
                    builder.Append(c);
                    continue;
                }

                if (AtEnd)
                {
                    throw new JsonException("unterminated escape");
                }

                var escape = text[at++];
                switch (escape)
                {
                    case '"':
                        builder.Append('"');
                        break;
                    case '\\':
                        builder.Append('\\');
                        break;
                    case '/':
                        builder.Append('/');
                        break;
                    case 'b':
                        builder.Append('\b');
                        break;
                    case 'f':
                        builder.Append('\f');
                        break;
                    case 'n':
                        builder.Append('\n');
                        break;
                    case 'r':
                        builder.Append('\r');
                        break;
                    case 't':
                        builder.Append('\t');
                        break;
                    case 'u':
                        if (at + 4 > text.Length)
                        {
                            throw new JsonException("truncated unicode escape");
                        }

                        builder.Append((char)int.Parse(
                            text.Substring(at, 4),
                            NumberStyles.HexNumber,
                            CultureInfo.InvariantCulture));
                        at += 4;
                        break;
                    default:
                        throw new JsonException("unknown escape `\\" + escape + "`");
                }
            }
        }

        private JsonValue ReadNumber()
        {
            var start = at;
            if (!AtEnd && text[at] == '-')
            {
                at++;
            }

            var integral = true;
            while (!AtEnd)
            {
                var c = text[at];
                if (c >= '0' && c <= '9')
                {
                    at++;
                    continue;
                }

                if (c == '.' || c == 'e' || c == 'E' || c == '+' || c == '-')
                {
                    integral = false;
                    at++;
                    continue;
                }

                break;
            }

            var slice = text.Substring(start, at - start);
            if (slice.Length == 0)
            {
                throw new JsonException("expected a number");
            }

            if (integral && long.TryParse(slice, NumberStyles.Integer, CultureInfo.InvariantCulture, out var asLong))
            {
                return JsonValue.Integer(asLong);
            }

            if (double.TryParse(slice, NumberStyles.Float, CultureInfo.InvariantCulture, out var asDouble))
            {
                return JsonValue.Number(asDouble);
            }

            throw new JsonException("`" + slice + "` is not a number");
        }
    }
}
