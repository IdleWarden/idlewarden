// SPDX-License-Identifier: MPL-2.0

using IdleWarden.Kale.Decisions;
using Xunit;

namespace IdleWarden.Kale.Decisions.Tests
{
    public class CastGateTests
    {
        private static CastRefusal Check(
            bool battle = true,
            bool known = true,
            bool target = true,
            double cooldown = 0.0,
            int cost = 10,
            int mana = 50,
            bool queued = false)
        {
            return CastGate.Check(battle, known, target, cooldown, cost, mana, queued);
        }

        [Fact]
        public void AClearCastIsNotRefused()
        {
            Assert.Equal(CastRefusal.None, Check());
        }

        [Fact]
        public void EachObstacleIsNamed()
        {
            Assert.Equal(CastRefusal.NoSpell, Check(known: false));
            Assert.Equal(CastRefusal.NoTarget, Check(target: false));
            Assert.Equal(CastRefusal.OnCooldown, Check(cooldown: 0.4));
            Assert.Equal(CastRefusal.NotEnoughMana, Check(cost: 60, mana: 59));
            Assert.Equal(CastRefusal.AlreadyQueued, Check(queued: true));
        }

        [Fact]
        public void ManaExactlyCoveringTheCostIsEnough()
        {
            Assert.Equal(CastRefusal.None, Check(cost: 50, mana: 50));
        }

        [Fact]
        public void AMissingSpellIsReportedBeforeTheSpellsOwnObstacles()
        {
            Assert.Equal(
                CastRefusal.NoSpell,
                Check(known: false, target: false, cooldown: 9.0, cost: 99, mana: 0, queued: true));
        }

        [Fact]
        public void NoBattleIsReportedBeforeAnythingElseBecauseTheGameChecksItFirst()
        {
            Assert.Equal(
                CastRefusal.NoBattle,
                Check(battle: false, known: false, target: false, cooldown: 9.0, cost: 99, mana: 0, queued: true));
            Assert.Equal(CastRefusal.NoBattle, Check(battle: false));
        }

        [Fact]
        public void EveryRefusalCarriesSomethingToShowTheUser()
        {
            foreach (CastRefusal refusal in System.Enum.GetValues(typeof(CastRefusal)))
            {
                var explained = CastGate.Explain(refusal, "Heal");
                if (refusal == CastRefusal.None)
                {
                    Assert.Null(explained);
                    continue;
                }
                Assert.False(string.IsNullOrWhiteSpace(explained));
                Assert.Contains("Heal", explained);
            }
        }
    }
}
