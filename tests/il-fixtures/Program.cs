using System;

public static class Fixture
{
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
            default: return 2;
        }
        Console.WriteLine(value);
        return 0;
    }
}
