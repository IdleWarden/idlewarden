// SPDX-License-Identifier: MPL-2.0

using System.Collections.Generic;

namespace IdleWarden.Kale.Decisions
{
    /// How a spell finds its target. A closed set on purpose: the game accepts a
    /// unit or nothing, so these three cover every spell it has, and which spell
    /// to cast when belongs in the plugin's rules rather than in here.
    public enum TargetPolicy
    {
        None,
        LowestHp,
        Down,
    }

    public sealed class CastPlan
    {
        private CastPlan(SpellSnapshot spell, UnitSnapshot target, bool needsTarget, CastRefusal refusal)
        {
            Spell = spell;
            Target = target;
            NeedsTarget = needsTarget;
            Refusal = refusal;
        }

        public SpellSnapshot Spell { get; }

        public UnitSnapshot Target { get; }

        public bool NeedsTarget { get; }

        public CastRefusal Refusal { get; }

        public bool Allowed => Refusal == CastRefusal.None;

        public static bool TryPolicy(string name, out TargetPolicy policy)
        {
            switch (name)
            {
                case null:
                case "none":
                    policy = TargetPolicy.None;
                    return true;
                case "lowest_hp":
                    policy = TargetPolicy.LowestHp;
                    return true;
                case "down":
                    policy = TargetPolicy.Down;
                    return true;
                default:
                    policy = TargetPolicy.None;
                    return false;
            }
        }

        public static CastPlan For(KaleSnapshot state, string spellName, TargetPolicy policy, double healBelow)
        {
            var spell = state.Spell(spellName);
            var target = Pick(state.Party, policy, healBelow);
            var needsTarget = policy != TargetPolicy.None;

            var refusal = CastGate.Check(
                state.BattleActive,
                spell != null,
                !needsTarget || target != null,
                spell?.CooldownRemaining ?? 0.0,
                spell?.ManaCost ?? 0,
                state.Mana,
                spell?.Queued ?? false);

            return new CastPlan(spell, target, needsTarget, refusal);
        }

        private static UnitSnapshot Pick(
            IReadOnlyList<UnitSnapshot> party,
            TargetPolicy policy,
            double healBelow)
        {
            switch (policy)
            {
                case TargetPolicy.LowestHp:
                    return HealChoice.MostHurt(party, healBelow);
                case TargetPolicy.Down:
                    return FirstDown(party);
                default:
                    return null;
            }
        }

        private static UnitSnapshot FirstDown(IReadOnlyList<UnitSnapshot> party)
        {
            foreach (var unit in party)
            {
                if (unit != null && !unit.Alive)
                {
                    return unit;
                }
            }
            return null;
        }
    }
}
