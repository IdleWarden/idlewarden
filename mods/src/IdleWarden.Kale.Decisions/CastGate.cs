// SPDX-License-Identifier: MPL-2.0

namespace IdleWarden.Kale.Decisions
{
    public enum CastRefusal
    {
        None,
        NoSpell,
        NoTarget,
        OnCooldown,
        NotEnoughMana,
        AlreadyQueued,
    }

    public static class CastGate
    {
        /// The game refuses a bad cast on its own, but it refuses it in the
        /// player's face: a chat error and a shaking mana bar. Asking first keeps
        /// those out of a session that ticks several times a second, and gives
        /// the host a reason to report instead of a silent no-op.
        public static CastRefusal Check(
            bool spellKnown,
            bool hasTarget,
            double cooldownRemaining,
            int manaCost,
            int manaAvailable,
            bool alreadyQueued)
        {
            if (!spellKnown)
            {
                return CastRefusal.NoSpell;
            }
            if (!hasTarget)
            {
                return CastRefusal.NoTarget;
            }
            if (cooldownRemaining > 0.0)
            {
                return CastRefusal.OnCooldown;
            }
            if (manaCost > manaAvailable)
            {
                return CastRefusal.NotEnoughMana;
            }
            return alreadyQueued ? CastRefusal.AlreadyQueued : CastRefusal.None;
        }

        public static string Explain(CastRefusal refusal, string spell)
        {
            switch (refusal)
            {
                case CastRefusal.None:
                    return null;
                case CastRefusal.NoSpell:
                    return "`" + spell + "` is not unlocked in this save";
                case CastRefusal.NoTarget:
                    return "nobody needs `" + spell + "` right now";
                case CastRefusal.OnCooldown:
                    return "`" + spell + "` is on cooldown";
                case CastRefusal.NotEnoughMana:
                    return "not enough mana for `" + spell + "`";
                case CastRefusal.AlreadyQueued:
                    return "`" + spell + "` is already queued";
                default:
                    return "`" + spell + "` cannot be cast";
            }
        }
    }
}
