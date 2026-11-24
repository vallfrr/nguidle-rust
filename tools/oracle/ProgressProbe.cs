using System;
using System.Collections;
using System.Globalization;
using System.IO;
using System.Reflection;
using BepInEx;
using HarmonyLib;
using UnityEngine;

[BepInPlugin("org.nguidle.reference.progress", "NGU progression probe", "1.0.0")]
public class ProgressProbe : BaseUnityPlugin
{
    private string output;
    private static bool Skip() { return false; }
    private void Awake()
    {
        output = Environment.GetEnvironmentVariable("NGU_PROGRESS_PROBE_OUTPUT");
        if (String.IsNullOrEmpty(output)) return;
        try
        {
            var harmony = new Harmony("org.nguidle.reference.progress");
            var skip = new HarmonyMethod(typeof(ProgressProbe).GetMethod("Skip", BindingFlags.NonPublic | BindingFlags.Static));
            foreach (string item in new string[] { "SteamManager:Awake", "OpenFileDialog:Update" })
            {
                var parts = item.Split(':');
                var type = AccessTools.TypeByName(parts[0]);
                var method = type == null ? null : AccessTools.Method(type, parts[1]);
                if (method == null) throw new Exception("Missing hook " + item);
                harmony.Patch(method, prefix: skip);
            }
            StartCoroutine(Measure());
        }
        catch (Exception error)
        {
            File.WriteAllText(output, "setup_error=" + error);
            Application.Quit();
        }
    }

    private IEnumerator Measure()
    {
        yield return new WaitForSecondsRealtime(3f);
        var menuType = AccessTools.TypeByName("MainMenuController");
        var charType = AccessTools.TypeByName("Character");
        var menus = Resources.FindObjectsOfTypeAll(menuType);
        var characters = Resources.FindObjectsOfTypeAll(charType);
        if (menus.Length != 1 || characters.Length != 1)
        {
            File.WriteAllText(output, "unexpected_object_count=" + menus.Length + "," + characters.Length);
            Application.Quit();
            yield break;
        }
        var menu = menus[0];
        var character = characters[0];
        AccessTools.Method(menuType, "finishMainMenu").Invoke(menu, null);
        float start = Time.realtimeSinceStartup;
        using (var writer = new StreamWriter(output, false))
        {
            writer.WriteLine("unity=" + Application.unityVersion);
            WriteSample(writer, charType, character, 0, start);
            yield return new WaitForSecondsRealtime(60f);
            WriteSample(writer, charType, character, 1, start);
        }
        Application.Quit();
    }

    private static void WriteSample(StreamWriter writer, Type type, object instance, int sample, float start)
    {
        var fields = new string[] { "curEnergy", "idleEnergy", "capEnergy", "energyGained", "energyBarProgress" };
        writer.Write(sample.ToString(CultureInfo.InvariantCulture) + "," +
            (Time.realtimeSinceStartup - start).ToString("R", CultureInfo.InvariantCulture));
        foreach (var field in fields)
        {
            var value = AccessTools.Field(type, field).GetValue(instance);
            writer.Write("," + Convert.ToString(value, CultureInfo.InvariantCulture));
        }
        writer.WriteLine();
        writer.Flush();
    }
}
