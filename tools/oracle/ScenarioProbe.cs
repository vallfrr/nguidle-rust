using System;
using System.Collections;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using BepInEx;
using HarmonyLib;
using UnityEngine;

// Opt-in reference recorder. Run only in the separately patched game copy.
[BepInPlugin("org.nguidle.reference.scenario", "NGU scenario reference", "1.0.0")]
public class ScenarioProbe : BaseUnityPlugin
{
    private IEnumerator Start()
    {
        string scenario = Environment.GetEnvironmentVariable("NGU_SCENARIO_PROBE_INPUT");
        string output = Environment.GetEnvironmentVariable("NGU_SCENARIO_PROBE_OUTPUT");
        if (String.IsNullOrEmpty(scenario) || String.IsNullOrEmpty(output)) yield break;
        yield return new WaitForSecondsRealtime(3f);
        var chars = Resources.FindObjectsOfTypeAll(AccessTools.TypeByName("Character"));
        var menus = Resources.FindObjectsOfTypeAll(AccessTools.TypeByName("MainMenuController"));
        if (chars.Length != 1 || menus.Length != 1) { File.WriteAllText(output, "unexpected object count"); Application.Quit(); yield break; }
        var character = chars[0];
        var type = character.GetType();
        AccessTools.Method(menus[0].GetType(), "finishMainMenu").Invoke(menus[0], null);
        string save = Environment.GetEnvironmentVariable("NGU_PROGRESS_PROBE_SAVE");
        if (!String.IsNullOrEmpty(save))
        {
            var dialogs = Resources.FindObjectsOfTypeAll(AccessTools.TypeByName("OpenFileDialog"));
            if (dialogs.Length != 1 || !(bool)AccessTools.Method(dialogs[0].GetType(), "quickLoad", new Type[] { typeof(string) }).Invoke(dialogs[0], new object[] { save }))
            { File.WriteAllText(output, "save_load_failed"); Application.Quit(); yield break; }
        }
        UnityEngine.Random.InitState(12345);
        var lines = new List<string>(File.ReadAllLines(scenario));
        float start = Time.realtimeSinceStartup;
        long previous = -1;
        using (var writer = new StreamWriter(output, false))
        {
            writer.WriteLine("scheduled_tick,elapsed,action,menu,energy,idle,cap,gained,bar");
            foreach (string raw in lines)
            {
                var line = raw.Trim();
                if (line.Length == 0 || line.StartsWith("#")) continue;
                var parts = line.Split(new char[] { ' ' }, 3, StringSplitOptions.RemoveEmptyEntries);
                long tick;
                if (parts.Length < 2 || !Int64.TryParse(parts[0], out tick) || tick < previous)
                { writer.WriteLine("error,invalid scenario"); break; }
                previous = tick;
                while (Time.realtimeSinceStartup - start < tick / 30f) yield return null;
                string action = parts[1];
                string argument = parts.Length == 3 ? parts[2].Trim() : "";
                if (action == "menu")
                {
                    int id;
                    if (!Int32.TryParse(argument, out id)) { writer.WriteLine("error,invalid menu"); break; }
                    var swapper = AccessTools.Field(type, "menuSwapper").GetValue(character);
                    AccessTools.Method(swapper.GetType(), "swapMenu").Invoke(swapper, new object[] { id });
                }
                else if (action == "check")
                {
                    if (argument.Length == 0 || argument.IndexOfAny(Path.GetInvalidFileNameChars()) >= 0 || argument.Contains(".."))
                    { writer.WriteLine("error,invalid check name"); break; }
                    var importer = AccessTools.Field(type, "importExport").GetValue(character);
                    var saveText = (string)AccessTools.Method(importer.GetType(), "getBase64Data").Invoke(importer, null);
                    File.WriteAllText(output + "." + argument + ".sav", saveText);
                }
                else if (action != "mark" && action != "end")
                { writer.WriteLine("error,unsupported action " + action); break; }
                writer.Write(tick + "," + (Time.realtimeSinceStartup - start).ToString("R", CultureInfo.InvariantCulture) + "," + action);
                foreach (string field in new string[] { "menuID", "curEnergy", "idleEnergy", "capEnergy", "energyGained", "energyBarProgress" })
                    writer.Write("," + Convert.ToString(AccessTools.Field(type, field).GetValue(character), CultureInfo.InvariantCulture));
                writer.WriteLine();
                writer.Flush();
                if (action == "end") break;
            }
        }
        Application.Quit();
    }
}
