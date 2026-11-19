"""读取编译请求，输出效果和来源记录。"""
import argparse
import hashlib
import json
from pathlib import Path

from .actions import Compiler
from .errors import Unsupported

def main() -> None:
    """Compile a request and write effects plus source/operation coverage evidence."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('request', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    request = json.loads(args.request.read_text(encoding='utf-8-sig'))
    files = [Path(p) for p in request['ability_files']]
    compiler = Compiler(files, request['bindings'], request['active_points'], request['hit_energy'])
    result = {'scope': 'selected_regular_skill_chain_only', 'entry': request['entry'],
              'inputs': [{'path': str(p), 'sha256': hashlib.sha256(p.read_bytes()).hexdigest()} for p in files]}
    try:
        result['effects'] = compiler.ability(request['entry'])
        result['status'] = 'compiled'
    except Unsupported as error:
        result.update(status='unsupported', error=str(error))
    result.update(operations=compiler.evidence, all_scanned_types=dict(compiler.types))
    args.output.write_text(json.dumps(result, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
    print(json.dumps({'status': result['status'], 'output': str(args.output), 'effects': len(result.get('effects', []))}))
    if result['status'] != 'compiled':
        raise SystemExit(2)


if __name__ == '__main__':
    main()
