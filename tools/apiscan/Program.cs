// Liste les membres externes (MemberRef) references par une assembly : API a fournir au port.
using System.Reflection.Metadata;
using System.Reflection.PortableExecutable;

var path = args[0];
using var fs = File.OpenRead(path);
using var pe = new PEReader(fs);
var md = pe.GetMetadataReader();
var rows = new SortedDictionary<string,int>();
string TypeName(EntityHandle h)
{
    switch (h.Kind)
    {
        case HandleKind.TypeReference:
            var tr = md.GetTypeReference((TypeReferenceHandle)h);
            var ns = md.GetString(tr.Namespace); var n = md.GetString(tr.Name);
            return (ns.Length > 0 ? ns + "." : "") + n;
        case HandleKind.TypeSpecification: return "<generic/spec>";
        default: return "<" + h.Kind + ">";
    }
}
foreach (var h in md.MemberReferences)
{
    var m = md.GetMemberReference(h);
    var key = TypeName(m.Parent) + "::" + md.GetString(m.Name);
    rows[key] = 0;
}
foreach (var r in rows.Keys) Console.WriteLine(r);
