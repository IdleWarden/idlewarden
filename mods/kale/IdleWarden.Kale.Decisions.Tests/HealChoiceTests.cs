// SPDX-License-Identifier: MPL-2.0

using IdleWarden.Kale.Decisions;
using Xunit;

namespace IdleWarden.Kale.Decisions.Tests
{
    public class HealChoiceTests
    {
        private static UnitSnapshot Unit(string name, double hp, double maxHp, bool alive = true)
        {
            return new UnitSnapshot(name, new Magnitude(hp, 0), new Magnitude(maxHp, 0), alive);
        }

        [Fact]
        public void TheWorstWoundedAliveUnitIsChosen()
        {
            var chosen = HealChoice.MostHurt(
                new[]
                {
                    Unit("Bagel", 80.0, 100.0),
                    Unit("Klepon", 20.0, 100.0),
                    Unit("Madeleine", 45.0, 100.0),
                },
                0.75);

            Assert.Equal("Klepon", chosen.Name);
        }

        [Fact]
        public void ADownedUnitIsNotAHealTarget()
        {
            var chosen = HealChoice.MostHurt(
                new[]
                {
                    Unit("corpse", 0.0, 100.0, alive: false),
                    Unit("hurt", 50.0, 100.0),
                },
                0.75);

            Assert.Equal("hurt", chosen.Name);
        }

        [Fact]
        public void AHealthyPartyIsLeftAlone()
        {
            var chosen = HealChoice.MostHurt(
                new[] { Unit("fine", 90.0, 100.0), Unit("full", 100.0, 100.0) },
                0.75);

            Assert.Null(chosen);
        }

        [Fact]
        public void AUnitTheSceneHasNotFinishedBuildingReadsAsUntouched()
        {
            var half_built = Unit("pending", 0.0, 0.0);

            Assert.Equal(1.0, HealChoice.Ratio(half_built));
            Assert.Null(HealChoice.MostHurt(new[] { half_built }, 0.75));
        }

        [Fact]
        public void TheLowestRatioIgnoresTheDeadOrItWouldStickAtZero()
        {
            var party = new[]
            {
                Unit("corpse", 0.0, 100.0, alive: false),
                Unit("scratched", 95.0, 100.0),
            };

            Assert.Equal(0.95, HealChoice.LowestRatio(party), 3);
            Assert.Equal(1, HealChoice.DownCount(party));
        }

        [Fact]
        public void AnOvershotHealDoesNotReportMoreThanFull()
        {
            Assert.Equal(1.0, HealChoice.Ratio(Unit("shielded", 150.0, 100.0)));
        }

        [Fact]
        public void HugeHealthPoolsStillCompare()
        {
            var chosen = HealChoice.MostHurt(
                new[]
                {
                    new UnitSnapshot("tank", new Magnitude(9.0, 30), new Magnitude(1.0, 31), true),
                    new UnitSnapshot("squishy", new Magnitude(1.0, 30), new Magnitude(1.0, 31), true),
                },
                0.95);

            Assert.Equal("squishy", chosen.Name);
        }
    }
}
