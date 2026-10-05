#!/usr/bin/env python3
"""Reproduce the small real-scan evaluation; no network or private input needed.
Requires Pillow and Playwright. Build Rust CLI + web, then run with a preview server.
"""
import argparse, hashlib, json, subprocess, tempfile
from pathlib import Path
from PIL import Image, ImageChops, ImageStat
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[1]
CORPUS = ROOT / 'crates/hexscope-raster/tests/fixtures/real-scans'
RECIPE = dict(schema='hexscope.film-recipe', version=1, algorithm='srgb-density-v1', settings=dict(mode='positive', crop=[0,0,0,0], exposure=0, contrast=1, baseColor=None))

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--base-url', default='http://127.0.0.1:4176')
    parser.add_argument('--output', default=str(ROOT/'docs/film-evaluation-results.json'))
    args = parser.parse_args()
    samples = json.loads((CORPUS/'manifest.json').read_text())['samples']
    workers = list((ROOT/'apps/web/dist/assets').glob('worker-*.js'))
    if len(workers) != 1: raise RuntimeError('build web first; exactly one worker is required')
    results = []
    with tempfile.TemporaryDirectory(prefix='hexscope-evaluation-') as temporary, sync_playwright() as p:
        folder=Path(temporary); recipe=folder/'recipe.json'; recipe.write_text(json.dumps(RECIPE))
        output=folder/'copies'
        command=[str(ROOT/'target/debug/hexscope'),'film','--recipe',str(recipe),'--out',str(output),'--format','tiff16']+[str(CORPUS/s['path']) for s in samples]
        run=subprocess.run(command,capture_output=True,text=True)
        if run.returncode: raise RuntimeError(run.stderr)
        # Folder enumeration is deterministic by canonical path, matching the CLI.
        samples.sort(key=lambda s: str((CORPUS/s['path']).resolve()))
        browser=p.chromium.launch(); page=browser.new_page(); page.goto(args.base_url)
        for index,sample in enumerate(samples):
            source=CORPUS/sample['path']; data=source.read_bytes()
            if hashlib.sha256(data).hexdigest()!=sample['sha256']: raise RuntimeError('source hash changed')
            scan=page.evaluate('''async ({url,bytes}) => {
              const worker=new Worker(url,{type:'module'});
              try { return await new Promise((resolve,reject)=>{
                const timer=setTimeout(()=>reject(new Error('frame inspection timed out')),30000);
                worker.onmessage=(e)=>{clearTimeout(timer);if(e.data.type==='codes')resolve(e.data.filmScan);else reject(new Error(e.data.message));};
                worker.onerror=(e)=>{clearTimeout(timer);reject(new Error(e.message));};
                worker.postMessage({id:1,type:'codes',picture:new Blob([new Uint8Array(bytes)],{type:'image/jpeg'}),inspectFilmScan:true});
              }); } finally {worker.terminate();}
            }''',dict(url=f'/assets/{workers[0].name}',bytes=list(data)))
            before=Image.open(source).convert('RGB'); after=Image.open(output/f'{index+1:06}-film.tif').convert('RGB')
            if before.size!=after.size: raise RuntimeError('neutral conversion changed size')
            difference=ImageStat.Stat(ImageChops.difference(before,after))
            results.append(dict(id=sample['id'],sha256=sample['sha256'],width=before.width,height=before.height,neutral_rgb_mean_absolute_error=round(sum(difference.mean)/3,5),frame=scan,expected_complete_frame=sample['expected_complete_frame'],complete_frame_false_positive=bool(scan.get('frameBounds')) and not sample['expected_complete_frame'],perforation_false_positive=bool(scan.get('perforationEdges'))))
        browser.close()
    report=dict(schema='hexscope.film-evaluation',version=1,corpus='four published positive Apollo 11 flight-film scans; not raw color negatives',samples=len(results),complete_frame_false_positives=sum(r['complete_frame_false_positive'] for r in results),perforation_false_positives=sum(r['perforation_false_positive'] for r in results),maximum_neutral_rgb_mae=max(r['neutral_rgb_mean_absolute_error'] for r in results),color_negative_accuracy='not_measured',ocr='not_implemented; needs labeled edge-text corpus',results=results)
    Path(args.output).write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps({k:v for k,v in report.items() if k!='results'},indent=2))
if __name__=='__main__': main()
