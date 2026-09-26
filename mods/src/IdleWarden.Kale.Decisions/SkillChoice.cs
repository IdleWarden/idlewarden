// SPDX-License-Identifier: MPL-2.0

using System.Collections.Generic;

namespace IdleWarden.Kale.Decisions
{
    public sealed class SkillCandidate
    {
        public SkillCandidate(string name, string currency, Magnitude cost, bool levelable)
            : this(name, currency, cost, levelable, "unknown", Magnitude.Zero, 0)
        {
        }

        public SkillCandidate(
            string name,
            string currency,
            Magnitude cost,
            bool levelable,
            string kind,
            Magnitude gain,
            int level = 0)
        {
            Level = level;
            Name = name;
            Currency = currency;
            Cost = cost;
            Levelable = levelable;
            Kind = kind;
            Gain = gain;
        }

        public string Name { get; }

        public string Currency { get; }

        public Magnitude Cost { get; }

        /// The game's own answer, not ours: affording a node is not the same as
        /// being allowed to take it, because a node also needs its requirements
        /// met. Taking price for permission buys nothing and reports success.
        public bool Levelable { get; }

        /// `Passive`, `ActiveAbility` or `Weapon`.
        public string Kind { get; }

        /// How many levels of this node the save already holds. Summed across the
        /// tree it is the only proof a purchase went through that holds for all
        /// three currencies.
        public int Level { get; }

        /// What the next level adds to this node's effect. Its unit is whatever
        /// the node measures, which is why it is only ever compared against nodes
        /// of the same kind paid in the same currency.
        public Magnitude Gain { get; }
    }

    public static class SkillChoice
    {
        public static SkillCandidate Cheapest(IEnumerable<SkillCandidate> candidates)
        {
            if (candidates == null)
            {
                return null;
            }

            SkillCandidate best = null;
            foreach (var candidate in candidates)
            {
                if (candidate == null || !candidate.Levelable)
                {
                    continue;
                }
                if (best == null || candidate.Cost.CompareTo(best.Cost) < 0)
                {
                    best = candidate;
                }
            }
            return best;
        }

        /// The most effect per unit spent, among nodes that measure the same thing
        /// and are paid for with the same currency.
        ///
        /// There is deliberately no ranking across those groups. This game gives
        /// no common denominator: one node buys maximum mana, another a critical
        /// chance, a third a gold multiplier, and the label describing the number
        /// is free text. Ordering "+30 mana for 4000 gold" against "+2% crit for 3
        /// skill points" would be a judgement the game does not encode, so the
        /// caller names the group it means.
        public static SkillCandidate BestGainPerCost(
            IEnumerable<SkillCandidate> candidates,
            string currency,
            string kind)
        {
            if (candidates == null)
            {
                return null;
            }

            SkillCandidate best = null;
            var bestScore = double.NegativeInfinity;
            foreach (var candidate in candidates)
            {
                if (candidate == null || !candidate.Levelable)
                {
                    continue;
                }
                if (candidate.Currency != currency || candidate.Kind != kind)
                {
                    continue;
                }
                if (candidate.Gain.IsZero || candidate.Cost.IsZero)
                {
                    continue;
                }

                var score = candidate.Gain.Log10Magnitude() - candidate.Cost.Log10Magnitude();
                if (best == null || score > bestScore)
                {
                    best = candidate;
                    bestScore = score;
                }
            }
            return best;
        }

        public static int LevelsOwned(IEnumerable<SkillCandidate> candidates)
        {
            if (candidates == null)
            {
                return 0;
            }

            var owned = 0;
            foreach (var candidate in candidates)
            {
                if (candidate != null && candidate.Level > 0)
                {
                    owned += candidate.Level;
                }
            }
            return owned;
        }

        public static int LevelableCount(IEnumerable<SkillCandidate> candidates)
        {
            if (candidates == null)
            {
                return 0;
            }

            var count = 0;
            foreach (var candidate in candidates)
            {
                if (candidate != null && candidate.Levelable)
                {
                    count++;
                }
            }
            return count;
        }
    }
}
