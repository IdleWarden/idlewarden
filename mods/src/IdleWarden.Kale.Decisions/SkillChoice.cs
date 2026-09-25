// SPDX-License-Identifier: MPL-2.0

using System.Collections.Generic;

namespace IdleWarden.Kale.Decisions
{
    public sealed class SkillCandidate
    {
        public SkillCandidate(string name, string currency, Magnitude cost, bool levelable)
        {
            Name = name;
            Currency = currency;
            Cost = cost;
            Levelable = levelable;
        }

        public string Name { get; }

        public string Currency { get; }

        public Magnitude Cost { get; }

        /// The game's own answer, not ours: affording a node is not the same as
        /// being allowed to take it, because a node also needs its requirements
        /// met. Taking price for permission buys nothing and reports success.
        public bool Levelable { get; }
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
