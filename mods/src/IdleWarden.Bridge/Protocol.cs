// SPDX-License-Identifier: Apache-2.0

using System;
using System.Collections.Generic;

namespace IdleWarden.Bridge
{
    /// <summary>
    /// A signal value, mirroring <c>idlewarden_plugin_api::Value</c> on the wire.
    /// </summary>
    public sealed class Value
    {
        private readonly JsonValue payload;

        private Value(string typeName, JsonValue payload)
        {
            TypeName = typeName;
            this.payload = payload;
        }

        public string TypeName { get; }

        public static Value Bool(bool value) => new Value("bool", JsonValue.Bool(value));

        public static Value Int(long value) => new Value("int", JsonValue.Integer(value));

        public static Value Float(double value) => new Value("float", JsonValue.Number(value));

        public static Value Ratio(double value)
        {
            if (value < 0.0 || value > 1.0)
            {
                throw new ArgumentOutOfRangeException(nameof(value), "a ratio lives in 0.0..=1.0");
            }

            return new Value("ratio", JsonValue.Number(value));
        }

        public static Value Text(string value) => new Value("text", JsonValue.String(value));

        public static Value Enum(string value) => new Value("enum", JsonValue.String(value));

        public static Value Point(double x, double y) => new Value("point", JsonValue.Object(
            new Dictionary<string, JsonValue>
            {
                ["x"] = JsonValue.Number(x),
                ["y"] = JsonValue.Number(y),
            }));

        public static Value Rect(double x, double y, double w, double h) => new Value("rect", JsonValue.Object(
            new Dictionary<string, JsonValue>
            {
                ["x"] = JsonValue.Number(x),
                ["y"] = JsonValue.Number(y),
                ["w"] = JsonValue.Number(w),
                ["h"] = JsonValue.Number(h),
            }));

        public JsonValue ToJson() => JsonValue.Object(new Dictionary<string, JsonValue>
        {
            ["type"] = JsonValue.String(TypeName),
            ["value"] = payload,
        });

        public static Value FromJson(JsonValue json)
        {
            var typeName = json.Member("type").AsString();
            return new Value(typeName, json.Member("value"));
        }

        public long AsInt() => payload.AsInteger();

        public double AsNumber() => payload.AsNumber();

        public bool AsBool() => payload.AsBool();

        public string AsString() => payload.AsString();
    }

    public sealed class Signal
    {
        public Signal(string id, Value value)
        {
            Id = id ?? throw new ArgumentNullException(nameof(id));
            Value = value ?? throw new ArgumentNullException(nameof(value));
        }

        public string Id { get; }

        public Value Value { get; }
    }

    public sealed class Intent
    {
        public Intent(string name, IReadOnlyDictionary<string, Value> parameters)
        {
            Name = name ?? throw new ArgumentNullException(nameof(name));
            Parameters = parameters ?? new Dictionary<string, Value>();
        }

        public string Name { get; }

        public IReadOnlyDictionary<string, Value> Parameters { get; }

        public Value Parameter(string name) =>
            Parameters.TryGetValue(name, out var found) ? found : null;

        public static Intent FromJson(JsonValue json)
        {
            var parameters = new Dictionary<string, Value>();
            var declared = json.MemberOrNull("params");
            if (declared != null && declared.Kind == JsonKind.Object)
            {
                foreach (var pair in declared.AsObject())
                {
                    parameters[pair.Key] = Value.FromJson(pair.Value);
                }
            }

            return new Intent(json.Member("name").AsString(), parameters);
        }
    }

    /// <summary>
    /// The result of executing one intent. This is a post-condition, not an
    /// acknowledgement: return <see cref="Succeeded"/> only after observing that
    /// the game state actually changed.
    /// </summary>
    public sealed class ActionOutcome
    {
        private readonly string outcome;
        private readonly string reason;
        private readonly long afterMs;

        private ActionOutcome(string outcome, string reason = null, long afterMs = 0)
        {
            this.outcome = outcome;
            this.reason = reason;
            this.afterMs = afterMs;
        }

        public static readonly ActionOutcome Succeeded = new ActionOutcome("succeeded");

        public static readonly ActionOutcome Aborted = new ActionOutcome("aborted");

        public static ActionOutcome Failed(string reason) => new ActionOutcome("failed", reason);

        public static ActionOutcome Rejected(string reason) => new ActionOutcome("rejected", reason);

        public static ActionOutcome TimedOut(long afterMs) =>
            new ActionOutcome("timed_out", afterMs: afterMs);

        public JsonValue ToJson()
        {
            var members = new Dictionary<string, JsonValue> { ["outcome"] = JsonValue.String(outcome) };
            if (reason != null)
            {
                members["reason"] = JsonValue.String(reason);
            }

            if (outcome == "timed_out")
            {
                members["after_ms"] = JsonValue.Integer(afterMs);
            }

            return JsonValue.Object(members);
        }
    }

    public enum RequestKind
    {
        Hello,
        Observe,
        Act,
    }

    public sealed class Request
    {
        private Request(RequestKind kind, string apiVersion = null, Intent intent = null)
        {
            Kind = kind;
            ApiVersion = apiVersion;
            Intent = intent;
        }

        public RequestKind Kind { get; }

        /// <summary>The host's API version. Only set for <see cref="RequestKind.Hello"/>.</summary>
        public string ApiVersion { get; }

        /// <summary>Only set for <see cref="RequestKind.Act"/>.</summary>
        public Intent Intent { get; }

        public static Request Parse(string line)
        {
            var json = JsonValue.Parse(line);
            var kind = json.Member("request").AsString();
            switch (kind)
            {
                case "hello":
                    return new Request(RequestKind.Hello, apiVersion: json.Member("api_version").AsString());
                case "observe":
                    return new Request(RequestKind.Observe);
                case "act":
                    return new Request(RequestKind.Act, intent: Intent.FromJson(json.Member("intent")));
                default:
                    throw new JsonException("unknown request `" + kind + "`");
            }
        }
    }

    public static class Response
    {
        public static string Hello(string pluginId, string apiVersion) =>
            JsonValue.Object(new Dictionary<string, JsonValue>
            {
                ["response"] = JsonValue.String("hello"),
                ["plugin"] = JsonValue.String(pluginId),
                ["api_version"] = JsonValue.String(apiVersion),
            }).ToString();

        public static string Observed(IEnumerable<Signal> signals)
        {
            var values = new List<JsonValue>();
            foreach (var signal in signals)
            {
                values.Add(JsonValue.Object(new Dictionary<string, JsonValue>
                {
                    ["id"] = JsonValue.String(signal.Id),
                    ["value"] = signal.Value.ToJson(),
                }));
            }

            return JsonValue.Object(new Dictionary<string, JsonValue>
            {
                ["response"] = JsonValue.String("observed"),
                ["signals"] = JsonValue.Array(values),
            }).ToString();
        }

        public static string Acted(ActionOutcome outcome) =>
            JsonValue.Object(new Dictionary<string, JsonValue>
            {
                ["response"] = JsonValue.String("acted"),
                ["outcome"] = outcome.ToJson(),
            }).ToString();

        public static string Error(string message) =>
            JsonValue.Object(new Dictionary<string, JsonValue>
            {
                ["response"] = JsonValue.String("error"),
                ["message"] = JsonValue.String(message ?? "unspecified failure"),
            }).ToString();
    }
}
