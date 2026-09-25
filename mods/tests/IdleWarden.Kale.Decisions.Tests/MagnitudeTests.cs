// SPDX-License-Identifier: MPL-2.0

using IdleWarden.Kale.Decisions;
using Xunit;

namespace IdleWarden.Kale.Decisions.Tests
{
    public class MagnitudeTests
    {
        [Fact]
        public void TheExponentIsPartOfTheValueAndNotDecoration()
        {
            var gold = new Magnitude(1.5, 30);

            Assert.True(
                System.Math.Abs(gold.ToDouble() / 1.5e30 - 1.0) < 1e-12,
                "the game keeps the mantissa and the exponent apart; returning the mantissa alone reads 1.5 for 1.5e30");
            Assert.NotEqual(1.5, gold.ToDouble());
        }

        [Fact]
        public void ComparingDoesNotFallBackToTheMantissa()
        {
            var smaller = new Magnitude(9.0, 3);
            var larger = new Magnitude(1.0, 6);

            Assert.True(smaller.CompareTo(larger) < 0, "9e3 is 9000 and 1e6 is a million");
            Assert.True(larger.CompareTo(smaller) > 0);
        }

        [Fact]
        public void OrderingSurvivesValuesNoDoubleCanHold()
        {
            var big = new Magnitude(1.0, 400);
            var bigger = new Magnitude(2.0, 400);

            Assert.True(double.IsInfinity(big.ToDouble()), "both overflow, so comparing doubles would tie");
            Assert.True(double.IsInfinity(bigger.ToDouble()));
            Assert.True(big.CompareTo(bigger) < 0);
        }

        [Fact]
        public void AValueBeyondADoubleAnnouncesItself()
        {
            Assert.True(new Magnitude(9.9, 300).FitsInDouble);
            Assert.False(new Magnitude(1.0, 400).FitsInDouble);
        }

        [Fact]
        public void ZeroCompares()
        {
            Assert.True(Magnitude.Zero.CompareTo(new Magnitude(1.0, 0)) < 0);
            Assert.Equal(0, Magnitude.Zero.CompareTo(new Magnitude(0.0, 12)));
        }
    }
}
