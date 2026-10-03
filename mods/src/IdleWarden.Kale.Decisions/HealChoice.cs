// SPDX-License-Identifier: MPL-2.0

using System.Collections.Generic;

namespace IdleWarden.Kale.Decisions
{
    public sealed class UnitSnapshot
    {
        public UnitSnapshot(string name, Magnitude hp, Magnitude maxHp, bool alive, Magnitude shield = default)
        {
            Name = name;
            Hp = hp;
            MaxHp = maxHp;
            Alive = alive;
            Shield = shield;
        }

        public string Name { get; }

        public Magnitude Hp { get; }

        public Magnitude MaxHp { get; }

        public bool Alive { get; }

        public Magnitude Shield { get; }

        public bool Shielded => Alive && Shield.CompareTo(Magnitude.Zero) > 0;
    }

    public static class HealChoice
    {
        /// A unit with no maximum reads as untouched rather than as a division by
        /// zero, so a unit the scene has not finished building is never mistaken
        /// for the one closest to death.
        public static double Ratio(UnitSnapshot unit)
        {
            if (unit == null)
            {
                return 1.0;
            }

            var max = unit.MaxHp.ToDouble();
            if (max <= 0.0 || double.IsNaN(max))
            {
                return 1.0;
            }

            var hp = unit.Hp.ToDouble();
            if (double.IsNaN(hp))
            {
                return 1.0;
            }

            var ratio = hp / max;
            if (ratio < 0.0)
            {
                return 0.0;
            }
            return ratio > 1.0 ? 1.0 : ratio;
        }

        public static UnitSnapshot MostHurt(IEnumerable<UnitSnapshot> units, double below)
        {
            if (units == null)
            {
                return null;
            }

            UnitSnapshot worst = null;
            var worstRatio = 0.0;
            foreach (var unit in units)
            {
                if (unit == null || !unit.Alive)
                {
                    continue;
                }

                var ratio = Ratio(unit);
                if (ratio >= below)
                {
                    continue;
                }
                if (worst == null || ratio < worstRatio)
                {
                    worst = unit;
                    worstRatio = ratio;
                }
            }
            return worst;
        }

        public static UnitSnapshot MostHurtUnshielded(IEnumerable<UnitSnapshot> units)
        {
            if (units == null)
            {
                return null;
            }

            UnitSnapshot chosen = null;
            var chosenRatio = 0.0;
            foreach (var unit in units)
            {
                if (unit == null || !unit.Alive || unit.Shielded)
                {
                    continue;
                }

                var ratio = Ratio(unit);
                if (chosen == null || ratio < chosenRatio)
                {
                    chosen = unit;
                    chosenRatio = ratio;
                }
            }
            return chosen;
        }

        public static int UnshieldedCount(IEnumerable<UnitSnapshot> units)
        {
            if (units == null)
            {
                return 0;
            }

            var count = 0;
            foreach (var unit in units)
            {
                if (unit != null && unit.Alive && !unit.Shielded)
                {
                    count++;
                }
            }
            return count;
        }

        public static int DownCount(IEnumerable<UnitSnapshot> units)
        {
            if (units == null)
            {
                return 0;
            }

            var count = 0;
            foreach (var unit in units)
            {
                if (unit != null && !unit.Alive)
                {
                    count++;
                }
            }
            return count;
        }

        public static double LowestRatio(IEnumerable<UnitSnapshot> units)
        {
            if (units == null)
            {
                return 1.0;
            }

            var lowest = 1.0;
            foreach (var unit in units)
            {
                if (unit == null || !unit.Alive)
                {
                    continue;
                }
                var ratio = Ratio(unit);
                if (ratio < lowest)
                {
                    lowest = ratio;
                }
            }
            return lowest;
        }
    }
}
