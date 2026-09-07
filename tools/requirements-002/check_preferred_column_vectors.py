#!/usr/bin/env python3
from pathlib import Path
import json, sys
ROOT=Path(__file__).resolve().parents[2]
V=ROOT/'Requirements/002/vectors/preferred-column.json'

def starts(t):
 s=[0]
 for i,c in enumerate(t):
  if c=='\n': s.append(i+1)
 return s

def off_to_lc(t,o):
 ss=starts(t); line=max(i for i,x in enumerate(ss) if x<=o); return line,o-ss[line]

def line_text(t,l): return t.split('\n')[l]

def dcol(s,n,w):
 c=0
 for ch in s[:n]: c += (w-(c%w)) if ch=='\t' else 1
 return c

def idx_for_goal(s,g,w):
 c=0
 for i,ch in enumerate(s):
  n=c+((w-(c%w)) if ch=='\t' else 1)
  if n>g: return i
  c=n
  if c==g: return i+1
 return len(s)

def simulate(v):
 t=v['document']; w=v['tab_width']; o=v['initial_char_offset']; pref=None; out=[]
 for k,op in enumerate(v['operations'],1):
  assert op=='down'
  l,ch=off_to_lc(t,o); cur=line_text(t,l)
  if pref is None: pref=dcol(cur,ch,w)
  tl=min(l+1,len(t.split('\n'))-1); target=line_text(t,tl); tc=idx_for_goal(target,pref,w)
  o=starts(t)[tl]+tc
  out.append({'after_operation':k,'char_offset':o,'logical_line':tl,'logical_character':tc,'display_column':dcol(target,tc,w),'preferred_display_column':pref})
 return out

def main():
 data=json.loads(V.read_text(encoding='utf-8')); bad=[]
 for v in data['vectors']:
  a=simulate(v); e=v['checkpoints']; label=f"{v['case_id']}::{v['variant_id']}"
  if a==e: print('PASS',label)
  else: bad.append((label,e,a))
 if bad:
  for label,e,a in bad:
   print('FAIL',label,file=sys.stderr); print(' expected=',e,file=sys.stderr); print(' actual=',a,file=sys.stderr)
  return 1
 return 0
if __name__=='__main__': raise SystemExit(main())
