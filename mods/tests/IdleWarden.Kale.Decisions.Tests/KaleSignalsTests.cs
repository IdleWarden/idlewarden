// SPDX-License-Identifier: MPL-2.0

using System.Collections.Generic;
using System.Linq;
using IdleWarden.Bridge;
using IdleWarden.Kale.Decisions;
using Xunit;

namespace IdleWarden.Kale.Decisions.Tests
{
    public class KaleSignalsTests
    {
        private static UnitSnapshot Unit(string name, double hp, double maxHp, bool alive = true)
        {
            return new UnitSnapshot(name, new Magnitude(hp, 0), new Magnitude(maxHp, 0), alive);
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

        private static KaleSnapshot State(
            IReadOnlyList<UnitSnapshot> party = null,
            IReadOnlyList<SpellSnapshot> spells = null,
            IReadOnlyList<SkillCandidate> nodes = null,
            Magnitude? gold = null,
            int mana = 100,
            int manaMax = 100,
            int tavernCompletion = 40,
            string scene = "BATTLE",
            int playtime = 120)
        {
            return new KaleSnapshot(
                scene,
                9999,
                gold ?? new Magnitude(1.0, 0),
                new Magnitude(5.0, 0),
                Magnitude.Zero,
                7,
                tavernCompletion,
                nodes,
                scene == "BATTLE",
                3,
                mana,
                manaMax,
                party ?? new[] { Unit("solo", 100.0, 100.0) },
                spells ?? new[] { Spell("Heal") },
                playtime);
        }

        private static Value Find(IReadOnlyList<Signal> signals, string id)
        {
            return signals.FirstOrDefault(signal => signal.Id == id)?.Value;
        }

        [Fact]
        public void TheTitleScreenSaysItsNumbersAreNotTheSaveYet()
        {
            var menu = KaleSignals.From(State(scene: "MAIN MENU", playtime: 0));

            Assert.False(
                Find(menu, "save.loaded").AsBool(),
                "before the save loads the game reports its defaults, one gold and no progress");
            Assert.Equal("main_menu", Find(menu, "ui.screen_id").AsString());

            var playing = KaleSignals.From(State(scene: "TAVERN", playtime: 4200));
            Assert.True(Find(playing, "save.loaded").AsBool());
            Assert.Equal("tavern", Find(playing, "ui.screen_id").AsString());
        }

        [Fact]
        public void NoSignalIdIsEmittedTwice()
        {
            var signals = KaleSignals.From(State(
                spells: new[] { Spell("Heal"), Spell("Party Heal"), Spell("Raise") }));

            var duplicates = signals
                .GroupBy(signal => signal.Id)
                .Where(group => group.Count() > 1)
                .Select(group => group.Key)
                .ToList();

            Assert.Empty(duplicates);
        }

        [Fact]
        public void AnOvershotHealDoesNotThrowWhileBuildingTheObservation()
        {
            var signals = KaleSignals.From(State(party: new[] { Unit("shielded", 150.0, 100.0) }));

            Assert.Equal(1.0, Find(signals, "party.lowest_hp_ratio").AsNumber());
            Assert.Equal(1.0, Find(signals, "party.average_hp_ratio").AsNumber());
        }

        [Fact]
        public void AHalfBuiltPartyDoesNotThrowEither()
        {
            var signals = KaleSignals.From(State(party: new[] { Unit("pending", 0.0, 0.0) }));

            Assert.Equal(1.0, Find(signals, "party.lowest_hp_ratio").AsNumber());
        }

        [Fact]
        public void ManaBeyondItsMaximumStillReportsAWellFormedRatio()
        {
            var signals = KaleSignals.From(State(mana: 700, manaMax: 480));

            Assert.Equal(1.0, Find(signals, "player.mana_ratio").AsNumber());
            Assert.Equal(700L, Find(signals, "player.mana").AsInt());
        }

        [Fact]
        public void ACompletionAboveAHundredIsStillARatio()
        {
            Assert.Equal(1.0, Find(KaleSignals.From(State(tavernCompletion: 140)), "tavern.completion").AsNumber());
            Assert.Equal(0.0, Find(KaleSignals.From(State(tavernCompletion: -3)), "tavern.completion").AsNumber());
        }

        [Fact]
        public void AResourceBeyondADoubleStaysFiniteOnTheWire()
        {
            var signals = KaleSignals.From(State(gold: new Magnitude(1.0, 400)));

            var gold = Find(signals, "resource.gold").AsNumber();
            Assert.False(double.IsInfinity(gold), "the host refuses a non-finite float, see #83");
            Assert.Equal(double.MaxValue, gold);
        }

        [Fact]
        public void ASpellWithASpaceGetsAnIdARuleCanName()
        {
            var signals = KaleSignals.From(State(spells: new[] { Spell("Party Heal") }));

            Assert.NotNull(Find(signals, "spell.party_heal.ready"));
            Assert.Equal(10L, Find(signals, "spell.party_heal.mana_cost").AsInt());
        }

        [Fact]
        public void ASpellIsReadyOnlyWhenTheGameWouldAcceptIt()
        {
            var ready = State(spells: new[] { Spell("Heal", mana: 10) }, mana: 50);
            Assert.True(Find(KaleSignals.From(ready), "spell.heal.ready").AsBool());

            var cooling = State(spells: new[] { Spell("Heal", cooldown: 1.2) }, mana: 50);
            Assert.False(Find(KaleSignals.From(cooling), "spell.heal.ready").AsBool());

            var broke = State(spells: new[] { Spell("Heal", mana: 40) }, mana: 20);
            Assert.False(Find(KaleSignals.From(broke), "spell.heal.ready").AsBool());

            var queued = State(spells: new[] { Spell("Heal", queued: true) }, mana: 50);
            Assert.False(Find(KaleSignals.From(queued), "spell.heal.ready").AsBool());
        }

        [Fact]
        public void TheReadyCountMatchesTheSpellsThatAreReady()
        {
            var signals = KaleSignals.From(State(
                spells: new[] { Spell("Heal"), Spell("Raise", cooldown: 4.0), Spell("Regen") },
                mana: 50));

            Assert.Equal(2L, Find(signals, "spell.ready_count").AsInt());
        }

        [Fact]
        public void TheLevelableCountIsWhatKeepsAnAgentFromBuyingIntoAWall()
        {
            var nodes = new[]
            {
                new SkillCandidate("locked", "Gold", new Magnitude(1.0, 0), false),
                new SkillCandidate("open", "Gold", new Magnitude(2.0, 0), true),
            };

            Assert.Equal(1L, Find(KaleSignals.From(State(nodes: nodes)), "tavern.levelable_nodes").AsInt());
        }
    }
}
