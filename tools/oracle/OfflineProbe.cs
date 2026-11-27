using System;
using System.Collections;
using System.Globalization;
using System.IO;
using System.Reflection;
using BepInEx;
using HarmonyLib;
using UnityEngine;

// Opens a save the way the game does when the player opens it (ImportExport.getSaveDataFromString, then
// OpenFileDialog.loadintoGame: offline progress and version updates apply), then writes the whole game state as a save
// (ImportExport.getBase64Data) at the requested times. Activates only when NGU_OFFLINE_PROBE_OUTPUT is set.
//   NGU_OFFLINE_PROBE_SAVE    the save to open
//   NGU_OFFLINE_PROBE_OUTPUT  output prefix: <prefix>.info.txt and <prefix>.<seconds>.sav
//   NGU_OFFLINE_PROBE_WAITS   comma-separated seconds after the load, default "0"
//   NGU_OFFLINE_PROBE_MENUS   comma-separated menu numbers to open after the load; the result of each goes to the info file
[BepInPlugin("org.nguidle.reference.offline", "NGU offline probe", "1.0.0")]
public class OfflineProbe : BaseUnityPlugin
{
    private string output;
    private static bool Skip() { return false; }

    private void Awake()
    {
        output = Environment.GetEnvironmentVariable("NGU_OFFLINE_PROBE_OUTPUT");
        if (String.IsNullOrEmpty(output)) return;
        try
        {
            var harmony = new Harmony("org.nguidle.reference.offline");
            var skip = new HarmonyMethod(typeof(OfflineProbe).GetMethod("Skip", BindingFlags.NonPublic | BindingFlags.Static));
            foreach (string item in new string[] { "SteamManager:Awake", "OpenFileDialog:Update" })
            {
                var parts = item.Split(':');
                var type = AccessTools.TypeByName(parts[0]);
                var method = type == null ? null : AccessTools.Method(type, parts[1]);
                if (method == null) throw new Exception("Missing hook " + item);
                harmony.Patch(method, prefix: skip);
            }
            StartCoroutine(Run());
        }
        catch (Exception error)
        {
            File.WriteAllText(output + ".info.txt", "setup_error=" + error);
            Application.Quit();
        }
    }

    private static object Only(string typeName)
    {
        var found = Resources.FindObjectsOfTypeAll(AccessTools.TypeByName(typeName));
        if (found.Length != 1) throw new Exception(typeName + " count " + found.Length);
        return found[0];
    }

    private IEnumerator Run()
    {
        yield return new WaitForSecondsRealtime(3f);
        string info = output + ".info.txt";
        string error = null;
        object character = null, exporter = null;
        try
        {
            var menu = Only("MainMenuController");
            character = Only("Character");
            AccessTools.Method(AccessTools.TypeByName("MainMenuController"), "finishMainMenu").Invoke(menu, null);
            exporter = Only("ImportExport");
        }
        catch (Exception e) { error = e.ToString(); }
        if (error != null) { File.WriteAllText(info, "setup_error=" + error); Application.Quit(); yield break; }
        yield return new WaitForSecondsRealtime(1f);
        float start = 0f;
        try
        {
            string text = File.ReadAllText(Environment.GetEnvironmentVariable("NGU_OFFLINE_PROBE_SAVE"));
            var decoded = AccessTools.Method(AccessTools.TypeByName("ImportExport"), "getSaveDataFromString").Invoke(exporter, new object[] { text });
            var dialog = Only("OpenFileDialog");
            double before = (DateTime.UtcNow - new DateTime(1970, 1, 1, 0, 0, 0, DateTimeKind.Utc)).TotalSeconds;
            bool ok = (bool)AccessTools.Method(AccessTools.TypeByName("OpenFileDialog"), "loadintoGame").Invoke(dialog, new object[] { decoded });
            double after = (DateTime.UtcNow - new DateTime(1970, 1, 1, 0, 0, 0, DateTimeKind.Utc)).TotalSeconds;
            start = Time.realtimeSinceStartup;
            File.WriteAllText(info, "unity=" + Application.unityVersion + "\nloaded=" + ok + "\nclock_before=" + before.ToString("R", CultureInfo.InvariantCulture) + "\nclock_after=" + after.ToString("R", CultureInfo.InvariantCulture) + "\n");
        }
        catch (Exception e) { error = e.ToString(); }
        if (error != null) { File.WriteAllText(info, "load_error=" + error); Application.Quit(); yield break; }
        string menus = Environment.GetEnvironmentVariable("NGU_OFFLINE_PROBE_MENUS");
        if (!String.IsNullOrEmpty(menus))
        {
            // Opens the listed menus (MenuSwapper.swapMenu) and records whether the game raised an exception.
            var swapper = Only("MenuSwapper");
            var swap = AccessTools.Method(AccessTools.TypeByName("MenuSwapper"), "swapMenu");
            foreach (string item in menus.Split(','))
            {
                try { swap.Invoke(swapper, new object[] { int.Parse(item, CultureInfo.InvariantCulture) }); File.AppendAllText(info, "menu " + item + ": ok" + Environment.NewLine); }
                catch (Exception e) { File.AppendAllText(info, "menu " + item + ": exception " + (e.InnerException ?? e).GetType().Name + Environment.NewLine); }
                yield return new WaitForSecondsRealtime(0.5f);
            }
        }
        string waits = Environment.GetEnvironmentVariable("NGU_OFFLINE_PROBE_WAITS");
        if (String.IsNullOrEmpty(waits)) waits = "0";
        foreach (string item in waits.Split(','))
        {
            float seconds = float.Parse(item, CultureInfo.InvariantCulture);
            float wait = start + seconds - Time.realtimeSinceStartup;
            if (wait > 0) yield return new WaitForSecondsRealtime(wait);
            try
            {
                var save = AccessTools.Method(AccessTools.TypeByName("ImportExport"), "getBase64Data").Invoke(exporter, null);
                File.WriteAllText(output + "." + item + ".sav", Convert.ToString(save));
                File.AppendAllText(info, "saved_at=" + (Time.realtimeSinceStartup - start).ToString("R", CultureInfo.InvariantCulture) + " for " + item + "\n");
            }
            catch (Exception e) { File.AppendAllText(info, "save_error=" + e + "\n"); }
        }
        Application.Quit();
    }
}
