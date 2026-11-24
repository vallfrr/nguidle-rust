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
            foreach (int seed in new int[] { 0, 1, -1, 123456789, Int32.MinValue, Int32.MaxValue })
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
                var systemRandom = new System.Random(seed);
                for (int i = 0; i < 16; i++)
                    writer.WriteLine("system_next," + seed + "," + i + "," +
                        systemRandom.Next().ToString(CultureInfo.InvariantCulture));
                systemRandom = new System.Random(seed);
                for (int i = 0; i < 16; i++)
                    writer.WriteLine("system_max," + seed + "," + i + "," +
                        systemRandom.Next(100).ToString(CultureInfo.InvariantCulture));
                systemRandom = new System.Random(seed);
                for (int i = 0; i < 16; i++)
                    writer.WriteLine("system_range," + seed + "," + i + "," +
                        systemRandom.Next(-100, 100).ToString(CultureInfo.InvariantCulture));
                systemRandom = new System.Random(seed);
                for (int i = 0; i < 16; i++)
                    writer.WriteLine("system_wide," + seed + "," + i + "," +
                        systemRandom.Next(Int32.MinValue, Int32.MaxValue).ToString(CultureInfo.InvariantCulture));
                systemRandom = new System.Random(seed);
                for (int i = 0; i < 16; i++)
                    writer.WriteLine("system_double," + seed + "," + i + "," +
                        systemRandom.NextDouble().ToString("R", CultureInfo.InvariantCulture));
            }
        }
        Application.Quit();
    }
}
