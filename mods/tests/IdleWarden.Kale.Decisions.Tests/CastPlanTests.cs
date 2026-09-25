// SPDX-License-Identifier: MPL-2.0

using System.Collections.Generic;
using IdleWarden.Kale.Decisions;
using Xunit;

namespace IdleWarden.Kale.Decisions.Tests
{
    public class CastPlanTests
    {
        private const double Below = 0.8;

        private static UnitSnapshot Unit(string name, double hp, bool alive = true)
        {
            return new UnitSnapshot(name, new Magnitude(hp, 0), new Magnitude(100.0, 0), alive);
        }

        private static KaleSnapshot State(
            IReadOnlyList<UnitSnapshot> party,
            IReadOnlyList<SpellSnapshot> spells,
            int mana = 100)
        {
            return new KaleSnapshot(
                "BATTLE", 9999,
                Magnitude.Zero, Magnitude.Zero, Magnitude.Zero,
                0, 0, null,
                true, 1, mana, 100,
                party, spells, 600);
        }

        private static SpellSnapshot Spell(
            string name,
            int mana = 10,
            double cooldown = 0.0,
            bool queued = false,
            bool canTargetDown = false)
        {
            return new SpellSnapshot(name, mana, cooldown, 1.0, canTargetDown, queued);
        }

        [Fact]
        public void AReviveTargetsTheMemberWhoIsDown()
        {
            var state = State(
                new[] { Unit("standing", 90.0), Unit("corpse", 0.0, alive: false) },
                new[] { Spell("Raise", mana: 40, canTargetDown: true) });

            var plan = CastPlan.For(state, "Raise", TargetPolicy.Down, Below);

            Assert.True(plan.Allowed);
            Assert.Equal("corpse", plan.Target.Name);
        }

        [Fact]
        public void AHealWithNobodyHurtIsRefusedForWantOfATargetRatherThanCastAtRandom()
        {
            var state = State(
                new[] { Unit("fine", 100.0) },
                new[] { Spell("Heal") });

            var plan = CastPlan.For(state, "Heal", TargetPolicy.LowestHp, Below);

            Assert.False(plan.Allowed);
            Assert.Equal(CastRefusal.NoTarget, plan.Refusal);
            Assert.Null(plan.Target);
        }

        [Fact]
        public void AGroupSpellNeedsNoTargetAndIsStillAllowedWithAHealthyParty()
        {
            var state = State(
                new[] { Unit("fine", 100.0) },
                new[] { Spell("Party Heal", mana: 20) });

            var plan = CastPlan.For(state, "Party Heal", TargetPolicy.None, Below);

            Assert.True(plan.Allowed);
            Assert.False(plan.NeedsTarget);
            Assert.Null(plan.Target);
        }

        [Fact]
        public void ASpellThisSaveHasNotUnlockedIsNamedAsMissing()
        {
            var state = State(new[] { Unit("hurt", 10.0) }, new[] { Spell("Heal") });

            var plan = CastPlan.For(state, "Time Warp", TargetPolicy.None, Below);

            Assert.Equal(CastRefusal.NoSpell, plan.Refusal);
        }

        [Fact]
        public void TheGameSideObstaclesStillApplyOnceATargetExists()
        {
            var hurt = new[] { Unit("hurt", 10.0) };

            Assert.Equal(
                CastRefusal.OnCooldown,
                CastPlan.For(State(hurt, new[] { Spell("Heal", cooldown: 1.5) }), "Heal", TargetPolicy.LowestHp, Below).Refusal);

            Assert.Equal(
                CastRefusal.NotEnoughMana,
                CastPlan.For(State(hurt, new[] { Spell("Heal", mana: 90) }, mana: 20), "Heal", TargetPolicy.LowestHp, Below).Refusal);

            Assert.Equal(
                CastRefusal.AlreadyQueued,
                CastPlan.For(State(hurt, new[] { Spell("Heal", queued: true) }), "Heal", TargetPolicy.LowestHp, Below).Refusal);
        }

        [Fact]
        public void AReviveWithEverybodyStandingHasNobodyToRaise()
        {
            var state = State(
                new[] { Unit("a", 50.0), Unit("b", 60.0) },
                new[] { Spell("Raise", canTargetDown: true) });

            Assert.Equal(
                CastRefusal.NoTarget,
                CastPlan.For(state, "Raise", TargetPolicy.Down, Below).Refusal);
        }

        [Fact]
        public void APolicyTheRulesMisspellIsReportedRatherThanTreatedAsNone()
        {
            Assert.True(CastPlan.TryPolicy("lowest_hp", out var lowest));
            Assert.Equal(TargetPolicy.LowestHp, lowest);

            Assert.True(CastPlan.TryPolicy("down", out var down));
            Assert.Equal(TargetPolicy.Down, down);

            Assert.True(CastPlan.TryPolicy(null, out var missing));
            Assert.Equal(TargetPolicy.None, missing);

            Assert.False(CastPlan.TryPolicy("lowest-hp", out _));
            Assert.False(CastPlan.TryPolicy("weakest", out _));
        }
    }
}
