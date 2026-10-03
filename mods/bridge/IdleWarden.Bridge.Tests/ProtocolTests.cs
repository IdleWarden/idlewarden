// SPDX-License-Identifier: MPL-2.0

using System.Collections.Generic;
using System.Globalization;
using System.Threading;
using IdleWarden.Bridge;
using Xunit;

namespace IdleWarden.Bridge.Tests
{
    public class ProtocolTests
    {
        [Fact]
        public void AHelloRequestCarriesTheHostApiVersion()
        {
            var request = Request.Parse("{\"request\":\"hello\",\"api_version\":\"0.1.0\"}");

            Assert.Equal(RequestKind.Hello, request.Kind);
            Assert.Equal("0.1.0", request.ApiVersion);
        }

        [Fact]
        public void AnObserveRequestNeedsNothingElse()
        {
            Assert.Equal(RequestKind.Observe, Request.Parse("{\"request\":\"observe\"}").Kind);
        }

        [Fact]
        public void AnActRequestCarriesTheIntentAndItsTypedParameters()
        {
            var line = "{\"request\":\"act\",\"intent\":{\"name\":\"buy_upgrade\","
                + "\"params\":{\"tier\":{\"type\":\"int\",\"value\":3}}}}";

            var request = Request.Parse(line);

            Assert.Equal(RequestKind.Act, request.Kind);
            Assert.Equal("buy_upgrade", request.Intent.Name);
            Assert.Equal(3, request.Intent.Parameter("tier").AsInt());
            Assert.Null(request.Intent.Parameter("missing"));
        }

        [Fact]
        public void AnIntentWithoutParametersIsNotAnError()
        {
            var request = Request.Parse("{\"request\":\"act\",\"intent\":{\"name\":\"click\"}}");

            Assert.Equal("click", request.Intent.Name);
            Assert.Empty(request.Intent.Parameters);
        }

        [Fact]
        public void AnUnknownRequestIsRejectedRatherThanIgnored()
        {
            Assert.Throws<JsonException>(() => Request.Parse("{\"request\":\"shutdown\"}"));
        }

        [Fact]
        public void AMalformedLineIsRejected()
        {
            Assert.Throws<JsonException>(() => Request.Parse("{\"request\":"));
        }

        [Fact]
        public void TheHelloResponseMatchesWhatTheHostDeserialises()
        {
            Assert.Equal(
                "{\"response\":\"hello\",\"plugin\":\"dev.example.game\",\"api_version\":\"^0.1\"}",
                Response.Hello("dev.example.game", "^0.1"));
        }

        [Fact]
        public void AnObservedResponseCarriesNoConfidence()
        {
            var signals = new List<Signal> { new Signal("resource.gold", Value.Int(42)) };

            var json = Response.Observed(signals);

            Assert.Equal(
                "{\"response\":\"observed\",\"signals\":"
                + "[{\"id\":\"resource.gold\",\"value\":{\"type\":\"int\",\"value\":42}}]}",
                json);
            Assert.DoesNotContain("confidence", json);
        }

        [Fact]
        public void EveryOutcomeSerialisesTheWayTheHostExpects()
        {
            Assert.Equal(
                "{\"response\":\"acted\",\"outcome\":{\"outcome\":\"succeeded\"}}",
                Response.Acted(ActionOutcome.Succeeded));

            Assert.Equal(
                "{\"response\":\"acted\",\"outcome\":{\"outcome\":\"failed\",\"reason\":\"broke\"}}",
                Response.Acted(ActionOutcome.Failed("broke")));

            Assert.Equal(
                "{\"response\":\"acted\",\"outcome\":{\"outcome\":\"timed_out\",\"after_ms\":250}}",
                Response.Acted(ActionOutcome.TimedOut(250)));
        }

        [Fact]
        public void AnErrorResponseNeverCarriesANullMessage()
        {
            Assert.Contains("unspecified failure", Response.Error(null));
        }

        [Fact]
        public void CompositeValuesUseTheContentShapeTheHostDeclares()
        {
            var point = Response.Observed(new[] { new Signal("cursor", Value.Point(0.5, 0.25)) });
            Assert.Contains("{\"type\":\"point\",\"value\":{\"x\":0.5,\"y\":0.25}}", point);

            var rect = Response.Observed(new[] { new Signal("roi", Value.Rect(0.1, 0.2, 0.3, 0.4)) });
            Assert.Contains("{\"type\":\"rect\",\"value\":{\"x\":0.1,\"y\":0.2,\"w\":0.3,\"h\":0.4}}", rect);
        }

        [Fact]
        public void ARatioOutsideItsRangeIsRefusedAtTheSource()
        {
            Assert.Throws<System.ArgumentOutOfRangeException>(() => Value.Ratio(1.5));
            Assert.Throws<System.ArgumentOutOfRangeException>(() => Value.Ratio(-0.1));
        }

        [Fact]
        public void NumbersAreWrittenInvariantlyWhateverTheGameLocaleIs()
        {
            var original = Thread.CurrentThread.CurrentCulture;
            try
            {
                Thread.CurrentThread.CurrentCulture = new CultureInfo("fr-FR");

                var json = Response.Observed(new[] { new Signal("ui.progress", Value.Ratio(0.5)) });

                Assert.Contains("0.5", json);
                Assert.DoesNotContain("0,5", json);
            }
            finally
            {
                Thread.CurrentThread.CurrentCulture = original;
            }
        }

        [Fact]
        public void AnIntegerNeverAcquiresADecimalPointOnTheWire()
        {
            var json = Response.Observed(new[] { new Signal("resource.gold", Value.Int(7)) });

            Assert.Contains("\"value\":7}", json);
            Assert.DoesNotContain("7.0", json);
        }
    }
}
