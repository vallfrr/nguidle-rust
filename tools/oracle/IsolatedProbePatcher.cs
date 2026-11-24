using System;
using System.Collections.Generic;
using Mono.Cecil;
using Mono.Cecil.Cil;

public static class IsolatedProbePatcher
{
    public static IEnumerable<string> TargetDLLs { get { return new[] { "Assembly-CSharp.dll" }; } }

    public static void Patch(AssemblyDefinition assembly)
    {
        Disable(assembly, "SteamManager", "Awake");
        Disable(assembly, "OpenFileDialog", "Update");
    }

    private static void Disable(AssemblyDefinition assembly, string typeName, string methodName)
    {
        var type = assembly.MainModule.GetType(typeName);
        if (type == null) throw new InvalidOperationException("Missing type: " + typeName);
        foreach (var method in type.Methods)
        {
            if (method.Name != methodName || method.Parameters.Count != 0) continue;
            method.Body.ExceptionHandlers.Clear();
            method.Body.Variables.Clear();
            method.Body.Instructions.Clear();
            method.Body.Instructions.Add(Instruction.Create(OpCodes.Ret));
            return;
        }
        throw new InvalidOperationException("Missing method: " + typeName + "." + methodName);
    }
}
