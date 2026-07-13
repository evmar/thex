import * as preact from 'preact';
import * as hooks from 'preact/hooks';
import data_json from './data.json';

interface DataJSON {
  name: string,
  instrs: Array<[number, string]>,
  blocks: Array<[number, Array<[[number], string]>]>,
}

function addr(ip: number) {
  return ip.toString(16).padStart(8, '0');
}

function Body() {
  const data = data_json as DataJSON;
  const [highlight, setHighlight] = hooks.useState([] as number[]);

  function hover(ips: number[]) {
    setHighlight(ips);
  }

  return <main>
    <div>{data.name}</div>
    <div style='display:flex; gap: 4em'>
    <pre>
      {data.instrs.map(([ip, instr]) => {
        let className;
        if (highlight.includes(ip)) {
          className = 'highlight';
        }
        return <div key={ip} class={className} onMouseOver={() => hover([ip])}>{addr(ip)} {instr}</div>;
      })}
    </pre>
    <pre>
      {data.blocks.map(([ip, stmts]) => {
        return <div>
          <div>{addr(ip)}:</div>
          {stmts.map(([ips, text]) => {
            let className;
            if (highlight.some((ip) => ips.includes(ip))) {
              className = 'highlight';
            }
            return <div class={className} onMouseOver={() => hover(ips)}>  {text}</div>;
          })}
        </div>;
      })}
      </pre>
  </div>
  </main>;
}

function main() {
  preact.render(<Body/>, document.body);
}

main();
