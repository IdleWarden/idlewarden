// SPDX-License-Identifier: MPL-2.0

// Signatures only, never behaviour. This assembly is named `Assembly-CSharp` so
// the Kale mod compiles against it where the game is not installed, and binds to
// the game's own assembly of that name when BepInEx loads it. Nothing here runs:
// every body throws, and the build refuses to copy this file's output next to the
// mod, because shipping it would shadow the real game code.
//
// Two rules hold this together. A type's base class must match the game's, or
// `x == null` compiles to reference equality where the game means Unity's
// overload, and the mod would read a destroyed object as present. And a member
// declared here as a property while the game declares a field compiles to a
// method call that does not exist at runtime, so fields stay fields.
//
// Drift is the risk this trades for: nothing here proves the game still looks
// like this. `dotnet build -p:KaleGameDir=...` compiles the same mod against the
// installed game instead, and that is the check.

using System;
using System.Collections.Generic;
using UnityEngine;

internal static class Stub
{
    internal const string Message =
        "IdleWarden.Kale.GameStubs carries signatures for compiling only; at runtime the game's own Assembly-CSharp answers";

    internal static Exception Only() => new NotSupportedException(Message);
}

public struct double_evrac
{
    public double _Value;

    public int _ZeroCount;

    public static double_evrac operator -(double_evrac a, double_evrac b) => throw Stub.Only();
}

public class Tuning
{
    public enum EnumCurrency
    {
        Gold,
        SkillPoint,
        Ruby,
    }
}

public class PlayerDatabase
{
    public static PlayerDatabase SharedPlayerDatabase;

    public int Playtime;

    public int GetIntSaveObject(string index) => throw Stub.Only();

    public double_evrac GetPlayerGold() => throw Stub.Only();

    public double_evrac GetPlayerSkillPoint() => throw Stub.Only();

    public double_evrac GetPlayerRuby() => throw Stub.Only();

    public double_evrac GetResource(Tuning.EnumCurrency currency) => throw Stub.Only();

    public int GetHighestLevelClear() => throw Stub.Only();

    public int GetPlayerMaxMana() => throw Stub.Only();
}

public class PopupTransition : MonoBehaviour
{
    public static PopupTransition SharedPopupTransition;

    public string GetActiveSceneName() => throw Stub.Only();
}

public class EngineBattle : MonoBehaviour
{
    public static EngineBattle SharedEngineBattle;

    public bool Isplaying;

    public int CurrentFloor;
}

public class EngineTavern : MonoBehaviour
{
    public static EngineTavern SharedEngineTavern;

    public void RefreshSkillTree() => throw Stub.Only();

    public int GetPercentageTavernCompletion() => throw Stub.Only();
}

public class SkillNode : ScriptableObject
{
    public enum EnumSkillNodeType
    {
        Passive,
        ActiveAbility,
        Weapon,
    }

    public string Name;

    public EnumSkillNodeType NodeType;

    public int MaxLevel;

    public Tuning.EnumCurrency CurrencyUsed;

    public double_evrac[] Price;

    public double_evrac[] SkillNumber;

    public double_evrac GetSkillNumber(int level) => throw Stub.Only();
}

public class PrefabSkillTree : MonoBehaviour
{
    public SkillNode ThisSkillNode;

    public bool CanLevelup;

    public int GetSkillNodeLevel() => throw Stub.Only();

    /// An override in the game, so this stays virtual: the call site then emits
    /// the same virtual dispatch it would against the real type.
    public virtual void FuncClick() => throw Stub.Only();
}

public class UnitHero
{
    public string GetName() => throw Stub.Only();
}

public class PrefabUnit : MonoBehaviour
{
    public UnitHero ThisUnitHero;

    public double_evrac CurrentHP;

    public double_evrac GetMaxHP() => throw Stub.Only();

    public bool IsAlive() => throw Stub.Only();
}

public class ActiveAbility
{
    public string Name;

    public float CurrentCooldown;

    public bool CanTargetDown;

    public virtual int GetManaCost() => throw Stub.Only();

    public virtual float GetCastingTime() => throw Stub.Only();
}

public class LibraryActiveAbility : MonoBehaviour
{
    public static LibraryActiveAbility SharedLibraryActiveAbility;

    public ActiveAbility GetActiveAbilityByName(string name) => throw Stub.Only();

    public List<ActiveAbility> GetAllUnlockedAbilities(bool skipNull = true) => throw Stub.Only();
}

public class PrefabHeroKale : MonoBehaviour
{
    public static PrefabHeroKale SharedPrefabHeroKale;

    public int CurrentMana;

    public bool HasQueueAbility(ActiveAbility ability) => throw Stub.Only();

    public void StartCastingSpell(ActiveAbility ability, PrefabUnit target) => throw Stub.Only();
}
