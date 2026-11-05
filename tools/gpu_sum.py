import sys, collections, re
import xml.etree.ElementTree as ET
t = ET.parse(sys.argv[1]).getroot()
vals = {el.get('id'): el for el in t.iter() if el.get('id') is not None}
r = lambda el: vals.get(el.get('ref'), el) if el.get('ref') is not None else el
tot=collections.defaultdict(float); cnt=collections.Counter()
for row in t.findall('.//row'):
    k=[r(c) for c in row]
    ch=k[2].get('fmt'); dur=float(k[1].text)
    lab=next((c.get('fmt') for c in k if c.tag=='metal-object-label'),'') or k[6].get('fmt','')
    lab=re.split(r'\s{2,}\(|\s\(earthdrop',lab)[0].split(':')[0]
    tot[(lab,ch)]+=dur; cnt[(lab,ch)]+=1
passes=max([c for (l,ch),c in cnt.items() if l=='main_opaque_pass_3d' and ch=='Fragment'] or [1])
frames=passes
if len(sys.argv)>2:
    # Real frames = trace window (s) x steady fps from the perf CSV (rows after load, pending == 0).
    import csv
    rows=[r for r in csv.DictReader(open(sys.argv[2])) if int(r['pending'])==0][2:]
    fps=sum(float(r['fps']) for r in rows)/max(len(rows),1)
    win=float(sys.argv[3]) if len(sys.argv)>3 else 4.0
    frames=max(fps*win,1)
    print(f"steady fps {fps:.1f} x {win}s window = {frames:.0f} frames; main_opaque intervals in trace: {passes}")
else:
    print("frames (main_opaque_pass_3d count):",frames)
per=collections.defaultdict(dict)
for (l,ch),v in tot.items(): per[l][ch]=v/1e6/frames
rows=sorted(per.items(), key=lambda x:-sum(x[1].values()))
iv=sorted((float(c[0].text),float(c[0].text)+float(c[1].text)) for c in [[r(x) for x in row] for row in t.findall('.//row')])
win=(max(e for _,e in iv)-iv[0][0]); u=0; cs,ce=iv[0]
for a,b in iv[1:]:
    if a>ce: u+=ce-cs; cs,ce=a,b
    else: ce=max(ce,b)
u+=ce-cs
print(f"GPU occupancy {100*u/win:.0f}% of the trace window; GPU time per frame {u/1e6/frames:.1f} ms (union of all intervals)")
allsum=sum(sum(d.values()) for d in per.values())
print("Per-pass numbers below are summed channel busy time, normalised to share of total; use shares, not absolute ms.")
print(f"{'label':44} {'frag':>6} {'vert':>6} {'comp':>6}  ms/frame (busy time per channel)")
for l,d in rows[:28]:
    print(f"{l[:44]:44} {d.get('Fragment',0):6.2f} {d.get('Vertex',0):6.2f} {d.get('Compute',0):6.2f}  {100*sum(d.values())/allsum:4.1f}%")
print("TOTAL", {ch: round(sum(d.get(ch,0) for d in per.values()),2) for ch in ('Fragment','Vertex','Compute')})
