// SPDX-License-Identifier: MPL-2.0

using System.Collections.Generic;
using System.Text;
using IdleWarden.Bridge;

namespace IdleWarden.Kale.Decisions
{
    public static class KaleSignals
    {
        /// What the plugin's rules are written against. Every id here is a
        /// promise: renaming one silently stops matching a rule, so they change
        /// with the plugin manifest or not at all.
        public static IReadOnlyList<Signal> From(KaleSnapshot state)
        {
            var signals = new List<Signal>
            {
                new Signal("ui.screen_id", Value.Enum(Slug(state.Scene ?? "unknown"))),
                new Signal("save.loaded", Value.Bool(state.SaveLoaded)),
                new Signal("progress.tutorial_done", Value.Bool(state.TutorialDone)),
                new Signal("progress.highest_level_clear", Value.Int(state.HighestLevelClear)),

                new Signal("resource.gold", Wire(state.Gold)),
                new Signal("resource.skill_points", Wire(state.SkillPoints)),
                new Signal("resource.ruby", Wire(state.Ruby)),

                new Signal("tavern.completion", Value.Ratio(Percent(state.TavernCompletion))),
                new Signal("tavern.levelable_nodes", Value.Int(SkillChoice.LevelableCount(state.Nodes))),
                new Signal("tavern.levels_owned", Value.Int(SkillChoice.LevelsOwned(state.Nodes))),
                new Signal("research.affordable", Value.Int(SkillChoice.LevelableCount(state.Research))),
                new Signal("research.levels_owned", Value.Int(SkillChoice.LevelsOwned(state.Research))),
                new Signal("training.affordable", Value.Int(SkillChoice.LevelableCount(state.Training))),
                new Signal("training.levels_owned", Value.Int(SkillChoice.LevelsOwned(state.Training))),

                new Signal("battle.active", Value.Bool(state.BattleActive)),
                new Signal("battle.floor", Value.Int(state.Floor)),

                new Signal("player.mana", Value.Int(state.Mana)),
                new Signal("player.mana_ratio", Value.Ratio(Share(state.Mana, state.ManaMax))),

                new Signal("party.size", Value.Int(state.Party.Count)),
                new Signal("party.down", Value.Int(HealChoice.DownCount(state.Party))),
                new Signal("party.lowest_hp_ratio", Value.Ratio(HealChoice.LowestRatio(state.Party))),
                new Signal("party.average_hp_ratio", Value.Ratio(AverageAliveRatio(state.Party))),
                new Signal("party.unshielded", Value.Int(HealChoice.UnshieldedCount(state.Party))),
            };

            var ready = 0;
            foreach (var spell in state.Spells)
            {
                if (spell == null || string.IsNullOrEmpty(spell.Name))
                {
                    continue;
                }

                var castable = Ready(spell, state.Mana);
                if (castable)
                {
                    ready++;
                }
                signals.Add(new Signal("spell." + Slug(spell.Name) + ".ready", Value.Bool(castable)));
                signals.Add(new Signal("spell." + Slug(spell.Name) + ".mana_cost", Value.Int(spell.ManaCost)));
            }
            signals.Add(new Signal("spell.ready_count", Value.Int(ready)));

            return signals;
        }

        /// `Party Heal` becomes `party_heal`, so a rule names a spell the way the
        /// game does without carrying its spaces and case into an id.
        public static string Slug(string name)
        {
            var slug = new StringBuilder(name.Length);
            foreach (var letter in name)
            {
                if (letter == ' ' || letter == '-' || letter == '\'')
                {
                    slug.Append('_');
                    continue;
                }
                if (char.IsLetterOrDigit(letter))
                {
                    slug.Append(char.ToLowerInvariant(letter));
                }
            }
            return slug.ToString();
        }

        private static bool Ready(SpellSnapshot spell, int mana)
        {
            return !spell.Queued && spell.CooldownRemaining <= 0.0 && spell.ManaCost <= mana;
        }

        private static double AverageAliveRatio(IReadOnlyList<UnitSnapshot> party)
        {
            var total = 0.0;
            var counted = 0;
            foreach (var unit in party)
            {
                if (unit == null || !unit.Alive)
                {
                    continue;
                }
                total += HealChoice.Ratio(unit);
                counted++;
            }
            return counted == 0 ? 1.0 : total / counted;
        }

        /// A resource inside a `double` keeps travelling as a float, so a host
        /// that predates the `big` value sees exactly what it always did. Past
        /// the ceiling (#83) it travels as a big value instead of being clamped
        /// to `double.MaxValue`, where `at_least` was always true and
        /// `increased` never was. A mantissa that is not a number has nothing
        /// to express and is clamped as before.
        private static Value Wire(Magnitude value)
        {
            if (value.FitsInDouble)
            {
                return Value.Float(value.ToDouble());
            }
            if (double.IsNaN(value.Mantissa) || double.IsInfinity(value.Mantissa))
            {
                return Value.Float(value.CompareTo(Magnitude.Zero) < 0 ? double.MinValue : double.MaxValue);
            }
            return Value.Big(value.Mantissa, value.Exponent);
        }

        private static double Percent(int whole)
        {
            if (whole <= 0)
            {
                return 0.0;
            }
            return whole >= 100 ? 1.0 : whole / 100.0;
        }

        private static double Share(int part, int whole)
        {
            if (whole <= 0 || part <= 0)
            {
                return 0.0;
            }
            return part >= whole ? 1.0 : (double)part / whole;
        }
    }
}
