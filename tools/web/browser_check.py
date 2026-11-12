#!/usr/bin/env python3
"""Exercise a served build using Firefox --headless --marionette (port 2828)."""
import argparse
import base64
import json
import socket
import time
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--url', default='http://127.0.0.1:8765/')
parser.add_argument('--output', type=Path, default=Path('target/browser-check'))
parser.add_argument('--actions', type=Path)
parser.add_argument('--skip-intro', action='store_true')
parser.add_argument('--expect-mode', choices=['menu', 'race'])
parser.add_argument('--seconds', type=int, default=45)
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=True)
sock = socket.create_connection(('127.0.0.1', 2828), timeout=60)
stream = sock.makefile('rb')

def receive():
    size = bytearray()
    while (c := stream.read(1)) != b':':
        if not c:
            raise RuntimeError('Firefox disconnected')
        size.extend(c)
    return json.loads(stream.read(int(size)))

receive()
sequence = 0

def command(name, params=None):
    global sequence
    sequence += 1
    data = json.dumps([0, sequence, name, params or {}]).encode()
    sock.sendall(str(len(data)).encode() + b':' + data)
    message = receive()
    if message[2]:
        raise RuntimeError(message[2])
    return message[3]

def script(text):
    return command('WebDriver:ExecuteScript', {'script': text, 'args': [], 'newSandbox': False, 'sandbox': None})['value']

try:
    command('WebDriver:NewSession', {'capabilities': {'alwaysMatch': {'acceptInsecureCerts': True}}})
    command('WebDriver:Navigate', {'url': args.url})
    script("window.__logs=[];const log=console.log;console.log=(...a)=>{window.__logs.push(a.map(String).join(' '));if(window.__logs.length>300)window.__logs.shift();log(...a)};window.__errors=[];const old=console.error;console.error=(...a)=>{window.__errors.push(a.map(String).join(' '));old(...a)};document.querySelector('#start')?.click()")
    if args.skip_intro:
        for _ in range(30):
            mode = script("return document.documentElement.dataset.gameMode")
            if mode == 'menu': break
            if mode == 'intro':
                command('WebDriver:PerformActions', {'actions':[{'type':'key','id':'keyboard','actions':[{'type':'keyDown','value':'\ue00c'},{'type':'keyUp','value':'\ue00c'}]}]})
            time.sleep(1)
    if args.actions:
        for index, action in enumerate(json.loads(args.actions.read_text())):
            if 'wait' in action:
                time.sleep(action['wait'])
            if 'click' in action:
                x, y = action['click']
                command('WebDriver:PerformActions', {'actions':[{'type':'pointer','id':'mouse','parameters':{'pointerType':'mouse'},'actions':[{'type':'pointerMove','duration':0,'x':x,'y':y,'origin':'viewport'},{'type':'pause','duration':100},{'type':'pointerDown','button':0},{'type':'pause','duration':100},{'type':'pointerUp','button':0}]}]})
            if 'touch-code' in action:
                code = json.dumps(action['touch-code'])
                position = script(f"document.querySelector('#touch').style.display='flex';const r=document.querySelector('[data-code='+{code}+']').getBoundingClientRect();return {{x:Math.round(r.x+r.width/2),y:Math.round(r.y+r.height/2)}}")
                command('WebDriver:PerformActions', {'actions':[{'type':'pointer','id':'finger','parameters':{'pointerType':'touch'},'actions':[{'type':'pointerMove','duration':0,'x':position['x'],'y':position['y'],'origin':'viewport'},{'type':'pointerDown','button':0},{'type':'pause','duration':round(action.get('seconds',1)*1000)},{'type':'pointerUp','button':0}]}]})
            if 'hold' in action:
                command('WebDriver:PerformActions', {'actions':[{'type':'key','id':'keyboard','actions':[{'type':'keyDown','value':action['hold']}]}]})
                time.sleep(action.get('seconds', 3))
                command('WebDriver:PerformActions', {'actions':[{'type':'key','id':'keyboard','actions':[{'type':'keyUp','value':action['hold']}]}]})
            if 'keys' in action:
                keys = []
                for char in action['keys']:
                    keys.extend([{'type':'keyDown','value':char},{'type':'keyUp','value':char}])
                command('WebDriver:PerformActions', {'actions':[{'type':'key','id':'keyboard','actions':keys}]})
            if 'script' in action:
                script(action['script'])
            result = command('WebDriver:TakeScreenshot', {'id': None, 'full': False})
            (args.output / f'action-{index}.png').write_bytes(base64.b64decode(result['value']))
    deadline = time.monotonic() + args.seconds
    while time.monotonic() < deadline:
        state = script("return {speed:document.documentElement.dataset.speed,mode:document.documentElement.dataset.gameMode,paused:document.documentElement.dataset.paused,status:document.querySelector('#status')?.textContent,loading:!document.querySelector('#loader')?.hidden,canvas:!!document.querySelector('canvas'),smoke:document.querySelector('#result')?.textContent,music:window.nfsVfs?[...window.nfsVfs.sparse.values()].map(s=>({total:s.size,resident:[...s.ranges.values()].reduce((n,b)=>n+b.length,0)})):[],errors:window.__errors,audio:window.__logs?.filter(line=>line.includes('audio')).slice(-6),logs:window.__logs?.slice(-6)}")
        print(json.dumps(state, ensure_ascii=False), flush=True)
        (args.output / 'state.json').write_text(json.dumps(state, ensure_ascii=False, indent=2))
        time.sleep(5)
    result = command('WebDriver:TakeScreenshot', {'id': None, 'full': False})
    (args.output / 'screen.png').write_bytes(base64.b64decode(result['value']))
    if state.get('errors'):
        raise RuntimeError(state['errors'])
    if args.expect_mode and state.get('mode') != args.expect_mode:
        raise RuntimeError(f"Expected {args.expect_mode}, got {state.get('mode')}")
finally:
    try:
        command('WebDriver:DeleteSession')
    finally:
        sock.close()
