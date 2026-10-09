#!/usr/bin/env python3
"""Independent scalar finite differences and all reverse-operation paths."""
import argparse
import math
from pathlib import Path
import shutil
import subprocess
import tempfile

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('binary',type=Path)
p.add_argument('--modules',type=Path,help='use source modules for pre-build library development')
p.add_argument('--case',action='append',help='run named cases only')
a=p.parse_args();binary=a.binary.resolve()
common='''import std.autodiff as ad;import std.autodiffAsync as jobs;import std.numeric as n;
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
fn scalar(value:Float)->FloatArray effects {} {let shape=List<Int>();shape.add(1);let cells=List<Float>();cells.add(value);return take(n.fromFloat(&shape,&cells));}
async fn compare(tape:ad.Tape,loss:ad.Node,node:ad.Node,expected:Float)->Bool effects {tasks} {
 let reference=take(ad.backward(&tape,loss));let answer=take(take(await spawn jobs.backward(move tape,loss)));
 let gradient=take(ad.gradient(&answer,node));assert_eq(gradient,take(ad.gradient(&reference,node)));
 match gradient{Some(array)=>{assert(take(n.math("abs",take(n.sum(&array))-expected))<0.000001);},None=>{panic("missing gradient");}}
 return true;
}
'''
functions={'relu':lambda x:max(0,x),'sigmoid':lambda x:1/(1+math.exp(-x)),'tanh':math.tanh,'exp':math.exp,'log':math.log,'square':lambda x:x*x}
cases=[]
for op,f in functions.items():
    for x in ([0.7,-0.7] if op=='relu' else [0.7]):
        h=1e-6;expected=(f(x+h)-f(x-h))/(2*h)
        cases.append((op+str(x),common+f'''var tape=ad.create();let parameter=take(ad.parameter(&mut tape,"x",scalar({x})));
let output=take(ad.unary(&mut tape,"{op}",parameter));assert(take(await spawn compare(move tape,output,parameter,{expected:.17f})));Out.println(true);publish;'''))
for op,expected in [('add',2.0),('sub',0.0),('mul',1.4)]:
    cases.append(('binary-'+op,common+f'''var tape=ad.create();let parameter=take(ad.parameter(&mut tape,"x",scalar(0.7)));let output=take(ad.binary(&mut tape,"{op}",parameter,parameter));assert(take(await spawn compare(move tape,output,parameter,{expected})));Out.println(true);publish;'''))
cases.append(('views-and-mean',common+'''var tape=ad.create();let parameter=take(ad.parameter(&mut tape,"x",scalar(0.7)));let dims=List<Int>();dims.add(2);dims.add(3);let broadcast=take(ad.broadcast(&mut tape,parameter,&dims));let transpose=take(ad.transpose(&mut tape,broadcast));let flat=List<Int>();flat.add(6);let reshape=take(ad.reshape(&mut tape,transpose,&flat));let loss=take(ad.mean(&mut tape,reshape));assert(take(await spawn compare(move tape,loss,parameter,1.0)));Out.println(true);publish;'''))
cases.append(('matmul-branches',common+'''let dims=List<Int>();dims.add(2);dims.add(2);let values=List<Float>();values.add(1.0);values.add(2.0);values.add(3.0);values.add(4.0);let matrix=take(n.fromFloat(&dims,&values));var tape=ad.create();let parameter=take(ad.parameter(&mut tape,"x",matrix));let product=take(ad.matmul(&mut tape,parameter,parameter));let loss=take(ad.sum(&mut tape,product));assert(take(await spawn compare(move tape,loss,parameter,40.0)));Out.println(true);publish;'''))
cases.append(('constant-and-errors',common+'''var tape=ad.create();let parameter=take(ad.parameter(&mut tape,"x",scalar(0.7)));let constant=take(ad.constant(&mut tape,scalar(2.0)));let loss=take(ad.binary(&mut tape,"mul",parameter,constant));let answer=take(take(await spawn jobs.backward(move tape,loss)));assert_eq(take(ad.gradient(&answer,constant)),None);
var other=ad.create();let foreign=take(ad.parameter(&mut other,"foreign",scalar(0.7)));match ad.gradient(&answer,foreign){Err(e)=>{assert_eq(e.code,"AutodiffNode");},_=>{panic("foreign key");}}
var empty=ad.create();match take(await spawn jobs.backward(move empty,foreign)){Err(e)=>{assert_eq(e.code,"AutodiffNode");},_=>{panic("empty tape");}}
let dims=List<Int>();dims.add(2);let vector=take(n.zerosFloat(&dims));var invalid=ad.create();let bad=take(ad.parameter(&mut invalid,"x",vector));match take(await spawn jobs.backward(move invalid,bad)){Err(e)=>{assert_eq(e.code,"AutodiffLoss");},_=>{panic("non-scalar loss");}}Out.println(true);publish;'''))
cases.append(('late-derivative-overflow',common+'''var tape=ad.create();let parameter=take(ad.parameter(&mut tape,"x",scalar(1e-320)));let loss=take(ad.unary(&mut tape,"log",parameter));let expected=ad.backward(&tape,loss);let observed=take(await spawn jobs.backward(move tape,loss));match move expected{Err(e)=>{assert_eq(e.code,"NumericOverflow");match move observed{Err(error)=>{assert_eq(error.code,e.code);},Ok(_)=>{panic("partial gradient");}}},Ok(_)=>{panic("expected overflow");}}Out.println(true);publish;'''))
cases.append(('constant-loss-unreachable',common+'''var tape=ad.create();let parameter=take(ad.parameter(&mut tape,"unused",scalar(0.7)));let loss=take(ad.constant(&mut tape,scalar(2.0)));let observed=take(take(await spawn jobs.backward(move tape,loss)));assert_eq(take(ad.gradient(&observed,parameter)),None);assert_eq(take(ad.gradient(&observed,loss)),None);Out.println(true);publish;'''))
if a.case:
    known={name for name,_ in cases}
    if any(name not in known for name in a.case):p.error('unknown case')
    cases=[case for case in cases if case[0] in a.case]
with tempfile.TemporaryDirectory(prefix='rewind-autodiff-check-') as d:
    root=Path(d)
    if a.modules:
        for module in a.modules.glob('*.rw'):shutil.copy2(module,root/module.name)
        version=subprocess.check_output([str(binary),'--version'],text=True).strip().split()[-1]
        (root/'rewind.toml').write_text(f'language = "{version}"\nsource_root = "."\nentry = "main.rw"\neffects = "tasks,output"\n')
    for name,source in cases:
        if a.modules:source=source.replace('import std.','import ')
        (root/'main.rw').write_text(source)
        if a.modules:
            update=subprocess.run([str(binary),'update','--root',str(root)],capture_output=True)
            if update.returncode:raise RuntimeError(name+': '+update.stderr.decode(errors='replace'))
        command=[str(binary),'run','main.rw','--native-work','100000000','--steps','10000000']
        if a.modules:command=[str(binary),'run','--root',str(root),'--native-work','100000000','--steps','10000000']
        result=subprocess.run(command,cwd=root,capture_output=True,timeout=90)
        if result.returncode:raise RuntimeError(name+': '+result.stderr.decode(errors='replace'))
        assert result.stdout.replace(b'\r\n',b'\n')==b'true\n',name
        print('PASS '+name,flush=True)
print('Independent finite differences, all derivative paths and typed failures passed')
