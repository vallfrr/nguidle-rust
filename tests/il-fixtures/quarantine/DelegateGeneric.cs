using System;

// Reproduction fixture: generic delegate returns are not yet generated.
public static class DelegateFixture
{
    public static int Run(int value)
    {
        Func<int, int> callback = n => n + 2;
        return callback(value);
    }
}
