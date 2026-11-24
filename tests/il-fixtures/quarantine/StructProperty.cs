// Reproduction fixture: generated Rust currently fails to compile (moved struct local).
public static class StructFixture
{
    public struct Counter { public int Value { get; set; } }
    public static int StructProperty(int value)
    {
        var counter = new Counter { Value = value };
        counter.Value += 4;
        return counter.Value;
    }
}
