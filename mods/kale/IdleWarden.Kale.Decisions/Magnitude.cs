// SPDX-License-Identifier: MPL-2.0

using System;

namespace IdleWarden.Kale.Decisions
{
    public readonly struct Magnitude : IComparable<Magnitude>, IEquatable<Magnitude>
    {
        public static readonly Magnitude Zero = new Magnitude(0.0, 0);

        public Magnitude(double mantissa, int exponent)
        {
            Mantissa = mantissa;
            Exponent = exponent;
        }

        public double Mantissa { get; }

        public int Exponent { get; }

        public bool IsZero => Mantissa == 0.0;

        public bool FitsInDouble
        {
            get
            {
                var value = ToDouble();
                return !double.IsInfinity(value) && !double.IsNaN(value);
            }
        }

        public double ToDouble()
        {
            return Mantissa * Math.Pow(10.0, Exponent);
        }

        public int CompareTo(Magnitude other)
        {
            if (IsZero || other.IsZero)
            {
                return Mantissa.CompareTo(other.Mantissa);
            }
            if (Mantissa < 0.0 != other.Mantissa < 0.0)
            {
                return Mantissa < 0.0 ? -1 : 1;
            }

            var mine = Scale();
            var theirs = other.Scale();
            return Mantissa < 0.0 ? theirs.CompareTo(mine) : mine.CompareTo(theirs);
        }

        public bool Equals(Magnitude other)
        {
            return CompareTo(other) == 0;
        }

        public override bool Equals(object obj)
        {
            return obj is Magnitude other && Equals(other);
        }

        public override int GetHashCode()
        {
            return IsZero ? 0 : Scale().GetHashCode();
        }

        public override string ToString()
        {
            return Exponent == 0 ? Mantissa.ToString("R") : Mantissa.ToString("R") + "e" + Exponent;
        }

        /// The base-ten scale of the value, so a ratio between two magnitudes can
        /// be ordered as a difference of scales and never overflow a `double`.
        public double Log10Magnitude()
        {
            return IsZero ? double.NegativeInfinity : Scale();
        }

        private double Scale()
        {
            return Math.Log10(Math.Abs(Mantissa)) + Exponent;
        }
    }
}
