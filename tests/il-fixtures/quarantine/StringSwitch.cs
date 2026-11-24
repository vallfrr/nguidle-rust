// Reproduction fixture: the generated code references an omitted hash helper.
public static class SwitchFixture
{
    public static int Pick(string key) => key switch
    {
        "alpha" => 1, "beta" => 2, "gamma" => 3, "delta" => 4,
        "epsilon" => 5, "zeta" => 6, "eta" => 7, "theta" => 8,
        "iota" => 9, "kappa" => 10, "lambda" => 11, "mu" => 12,
        _ => -1
    };
}
