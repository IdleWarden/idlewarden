// SPDX-License-Identifier: MPL-2.0

using IdleWarden.Kale.Decisions;
using Xunit;

namespace IdleWarden.Kale.Decisions.Tests
{
    public class SkillChoiceTests
    {
        private static SkillCandidate Node(string name, double mantissa, int exponent, bool levelable)
        {
            return new SkillCandidate(name, "Gold", new Magnitude(mantissa, exponent), levelable);
        }

        [Fact]
        public void ANodeTheGameRefusesIsSkippedEvenWhenItIsTheCheapest()
        {
            var chosen = SkillChoice.Cheapest(new[]
            {
                Node("Paycheck", 4.0, 3, false),
                Node("Top Priority", 1.85, 5, true),
            });

            Assert.NotNull(chosen);
            Assert.Equal("Top Priority", chosen.Name);
        }

        [Fact]
        public void TheCheapestLevelableNodeWinsAcrossExponents()
        {
            var chosen = SkillChoice.Cheapest(new[]
            {
                Node("expensive", 1.0, 12, true),
                Node("cheap", 5.0, 2, true),
                Node("middling", 7.0, 6, true),
            });

            Assert.Equal("cheap", chosen.Name);
        }

        [Fact]
        public void NothingLevelableMeansNoChoiceRatherThanAWastedClick()
        {
            Assert.Null(SkillChoice.Cheapest(new SkillCandidate[0]));
            Assert.Null(SkillChoice.Cheapest(new[] { Node("locked", 1.0, 0, false) }));
            Assert.Null(SkillChoice.Cheapest(null));
        }

        [Fact]
        public void TheLevelableCountIgnoresWhatTheGameRefuses()
        {
            var nodes = new[]
            {
                Node("a", 1.0, 0, true),
                Node("b", 1.0, 0, false),
                Node("c", 1.0, 0, true),
            };

            Assert.Equal(2, SkillChoice.LevelableCount(nodes));
        }
    }
}
