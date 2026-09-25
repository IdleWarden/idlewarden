// SPDX-License-Identifier: MPL-2.0

using IdleWarden.Kale.Decisions;
using Xunit;

namespace IdleWarden.Kale.Decisions.Tests
{
    public class GainPerCostTests
    {
        private static SkillCandidate Node(
            string name,
            double cost,
            double gain,
            string currency = "Gold",
            string kind = "Passive",
            bool levelable = true)
        {
            return new SkillCandidate(
                name,
                currency,
                new Magnitude(cost, 0),
                levelable,
                kind,
                new Magnitude(gain, 0));
        }

        [Fact]
        public void TheNodeReturningMostPerUnitSpentWins()
        {
            var best = SkillChoice.BestGainPerCost(
                new[]
                {
                    Node("meagre", cost: 100.0, gain: 1.0),
                    Node("generous", cost: 200.0, gain: 50.0),
                    Node("dear", cost: 5000.0, gain: 60.0),
                },
                "Gold",
                "Passive");

            Assert.Equal("generous", best.Name);
        }

        [Fact]
        public void NodesPaidInAnotherCurrencyAreNotInTheRace()
        {
            var best = SkillChoice.BestGainPerCost(
                new[]
                {
                    Node("gold node", cost: 1000.0, gain: 1.0),
                    Node("points node", cost: 1.0, gain: 100.0, currency: "SkillPoint"),
                },
                "Gold",
                "Passive");

            Assert.Equal("gold node", best.Name);
        }

        [Fact]
        public void NodesMeasuringSomethingElseAreNotInTheRaceEither()
        {
            var best = SkillChoice.BestGainPerCost(
                new[]
                {
                    Node("passive", cost: 1000.0, gain: 1.0),
                    Node("weapon", cost: 1.0, gain: 100.0, kind: "Weapon"),
                },
                "Gold",
                "Passive");

            Assert.Equal("passive", best.Name);
        }

        [Fact]
        public void ANodeTheGameRefusesIsNotRankedAtAll()
        {
            var best = SkillChoice.BestGainPerCost(
                new[]
                {
                    Node("locked bargain", cost: 1.0, gain: 1000.0, levelable: false),
                    Node("open", cost: 100.0, gain: 1.0),
                },
                "Gold",
                "Passive");

            Assert.Equal("open", best.Name);
        }

        [Fact]
        public void RatiosBeyondWhatADoubleHoldsStillOrder()
        {
            var best = SkillChoice.BestGainPerCost(
                new[]
                {
                    new SkillCandidate("huge cost", "Gold", new Magnitude(1.0, 400), true, "Passive", new Magnitude(1.0, 402)),
                    new SkillCandidate("better", "Gold", new Magnitude(1.0, 400), true, "Passive", new Magnitude(1.0, 405)),
                },
                "Gold",
                "Passive");

            Assert.Equal("better", best.Name);
        }

        [Fact]
        public void ANodeWithNoMeasuredGainIsSkippedRatherThanRankedAsInfinite()
        {
            var best = SkillChoice.BestGainPerCost(
                new[]
                {
                    Node("unmeasured", cost: 1.0, gain: 0.0),
                    Node("measured", cost: 900.0, gain: 1.0),
                },
                "Gold",
                "Passive");

            Assert.Equal("measured", best.Name);
        }

        [Fact]
        public void NothingComparableMeansNoRecommendation()
        {
            Assert.Null(SkillChoice.BestGainPerCost(new SkillCandidate[0], "Gold", "Passive"));
            Assert.Null(SkillChoice.BestGainPerCost(null, "Gold", "Passive"));
        }
    }
}
