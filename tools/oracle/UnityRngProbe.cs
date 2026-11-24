using System;
using System.Globalization;
using System.IO;
using BepInEx;
using UnityEngine;

[BepInPlugin("org.nguidle.reference.rng", "NGU Unity RNG reference", "1.0.0")]
public class UnityRngProbe : BaseUnityPlugin
{
    private void Awake()
    {
        string output = Environment.GetEnvironmentVariable("NGU_RNG_PROBE_OUTPUT");
        if (String.IsNullOrEmpty(output)) return;
        using (var writer = new StreamWriter(output, false))
        {
            writer.WriteLine("unity=" + Application.unityVersion);
            foreach (int seed in new int[] { 0, 1, -1, 123456789 })
            {
                UnityEngine.Random.InitState(seed);
                for (int i = 0; i < 16; i++)
                    writer.WriteLine("value," + seed + "," + i + "," +
                        UnityEngine.Random.value.ToString("R", CultureInfo.InvariantCulture));
                UnityEngine.Random.InitState(seed);
                for (int i = 0; i < 16; i++)
                    writer.WriteLine("range_int," + seed + "," + i + "," +
                        UnityEngine.Random.Range(-100, 100).ToString(CultureInfo.InvariantCulture));
                UnityEngine.Random.InitState(seed);
                for (int i = 0; i < 16; i++)
                    writer.WriteLine("range_float," + seed + "," + i + "," +
                        UnityEngine.Random.Range(-10f, 30f).ToString("R", CultureInfo.InvariantCulture));
            }
        }
        Application.Quit();
    }
}
