using System;

public static class Fixture
{
    public static int DelegateLambda(int value)
    {
        int offset = value % 3;
        Func<int, int> callback = n => n + offset + 2;
        return callback(value);
    }

    public static int DelegateBool(int value)
    {
        Func<int, bool> isPositive = n => n > 0;
        return isPositive(value) ? 1 : 0;
    }

    public static int StringSwitch(int value)
    {
        string key = value.ToString();
        return key switch {
            "0" => 1, "1" => 2, "2" => 3, "3" => 4, "4" => 5, "5" => 6,
            "6" => 7, "7" => 8, "8" => 9, "9" => 10, "10" => 11, "11" => 12,
            _ => -1
        };
    }

    public static int Branch(int value)
    {
        if (value < 0) return -value + 3;
        if (value == 0) return 7;
        return value * 2;
    }

    public static int Loop(int count)
    {
        int result = 0;
        for (int i = 0; i < count; i++) result += i;
        return result;
    }

    public static int Choice(int value)
    {
        switch (value)
        {
            case 0: return 11;
            case 1: return 31;
            case 2: return 47;
            default: return -9;
        }
    }

    public static int SeededRandom(int seed)
    {
        var random = new Random(seed);
        unchecked
        {
            return random.Next() ^ random.Next(100) ^ random.Next(-100, 100);
        }
    }

    public static int ByRef(int value)
    {
        Add(ref value);
        return value;
    }

    private static void Add(ref int value) { value += 3; }

    public static int Exception(int value)
    {
        try { if (value == 0) throw new ArgumentException(); return 10 / value; }
        catch (ArgumentException) { return -7; }
        finally { value++; }
    }

    public static int Main(string[] args)
    {
        int argument = int.Parse(args[1]);
        int value;
        switch (args[0])
        {
            case "Branch": value = Branch(argument); break;
            case "Loop": value = Loop(argument); break;
            case "Choice": value = Choice(argument); break;
            case "SeededRandom": value = SeededRandom(argument); break;
            case "ByRef": value = ByRef(argument); break;
            case "Exception": value = Exception(argument); break;
            case "StringSwitch": value = StringSwitch(argument); break;
            case "DelegateLambda": value = DelegateLambda(argument); break;
            case "DelegateBool": value = DelegateBool(argument); break;
            default: return 2;
        }
        Console.WriteLine(value);
        return 0;
    }
}
