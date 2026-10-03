// SPDX-License-Identifier: MPL-2.0

using System.Collections.Generic;
using IdleWarden.Bridge;
using Xunit;

namespace IdleWarden.Bridge.Tests
{
    public class JsonTests
    {
        [Fact]
        public void IntegersAndFloatsStaySeparate()
        {
            Assert.Equal(JsonKind.Integer, JsonValue.Parse("42").Kind);
            Assert.Equal(JsonKind.Number, JsonValue.Parse("42.0").Kind);
            Assert.Equal(JsonKind.Number, JsonValue.Parse("4e2").Kind);
            Assert.Equal(-7, JsonValue.Parse("-7").AsInteger());
        }

        [Fact]
        public void AnIntegerIsReadableAsANumberButNotTheOtherWayRound()
        {
            Assert.Equal(42.0, JsonValue.Parse("42").AsNumber());
            Assert.Throws<JsonException>(() => JsonValue.Parse("42.5").AsInteger());
        }

        [Fact]
        public void EscapesSurviveARoundTrip()
        {
            var original = "quote \" backslash \\ newline \n tab \t";

            var round = JsonValue.Parse(JsonValue.String(original).ToString()).AsString();

            Assert.Equal(original, round);
        }

        [Fact]
        public void ControlCharactersAreEscapedRatherThanEmittedRaw()
        {
            var json = JsonValue.String("a\u0001b").ToString();

            Assert.Equal("\"a\\u0001b\"", json);
        }

        [Fact]
        public void ANewlineInsideAStringNeverBreaksTheLineFraming()
        {
            var json = JsonValue.String("two\nlines").ToString();

            Assert.DoesNotContain("\n", json);
        }

        [Fact]
        public void UnicodeEscapesAreDecoded()
        {
            Assert.Equal("é", JsonValue.Parse("\"\\u00e9\"").AsString());
        }

        [Fact]
        public void NestedStructuresParse()
        {
            var json = JsonValue.Parse("{\"a\":[1,{\"b\":true},null],\"c\":{}}");

            Assert.Equal(3, json.Member("a").AsArray().Count);
            Assert.True(json.Member("a").AsArray()[1].Member("b").AsBool());
            Assert.Equal(JsonKind.Null, json.Member("a").AsArray()[2].Kind);
            Assert.Empty(json.Member("c").AsObject());
        }

        [Fact]
        public void AMissingFieldIsNamedInTheError()
        {
            var json = JsonValue.Parse("{\"a\":1}");

            var error = Assert.Throws<JsonException>(() => json.Member("b"));
            Assert.Contains("b", error.Message);
            Assert.Null(json.MemberOrNull("b"));
        }

        [Theory]
        [InlineData("{")]
        [InlineData("{\"a\"}")]
        [InlineData("[1,]")]
        [InlineData("\"unterminated")]
        [InlineData("tru")]
        [InlineData("{} trailing")]
        [InlineData("\"bad \\q escape\"")]
        public void MalformedInputIsRejected(string text)
        {
            Assert.Throws<JsonException>(() => JsonValue.Parse(text));
        }

        [Fact]
        public void RunawayNestingIsRefusedRatherThanOverflowingTheStack()
        {
            var deep = new string('[', 200) + new string(']', 200);

            Assert.Throws<JsonException>(() => JsonValue.Parse(deep));
        }

        [Fact]
        public void ObjectsKeepInsertionOrderSoResponsesAreStable()
        {
            var members = new Dictionary<string, JsonValue>
            {
                ["z"] = JsonValue.Integer(1),
                ["a"] = JsonValue.Integer(2),
            };

            Assert.Equal("{\"z\":1,\"a\":2}", JsonValue.Object(members).ToString());
        }
    }
}
