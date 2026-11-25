import { snippetCompletion, type Completion, type CompletionContext, type CompletionResult } from '@codemirror/autocomplete';

type Lang = 'java' | 'python' | 'ts' | 'yaml';

const t = (label: string, detail: string, template: string): Completion =>
  snippetCompletion(template, { label, detail, type: 'snippet', boost: 99 });

const JAVA: Completion[] = [
  t('getm', 'Spring @GetMapping', '@GetMapping("${/path}")\npublic ResponseEntity<${Object}> ${get}(${}) {\n\t${}\n\treturn ResponseEntity.ok(${});\n}'),
  t('postm', 'Spring @PostMapping', '@PostMapping("${/path}")\npublic ResponseEntity<${Object}> ${create}(@RequestBody ${Object} body) {\n\t${}\n\treturn ResponseEntity.status(HttpStatus.CREATED).body(${body});\n}'),
  t('putm', 'Spring @PutMapping', '@PutMapping("/${path}/{id}")\npublic ResponseEntity<${Object}> ${update}(@PathVariable ${String} id, @RequestBody ${Object} body) {\n\t${}\n\treturn ResponseEntity.ok(${body});\n}'),
  t('delm', 'Spring @DeleteMapping', '@DeleteMapping("/${path}/{id}")\npublic ResponseEntity<Void> ${delete}(@PathVariable ${String} id) {\n\t${}\n\treturn ResponseEntity.noContent().build();\n}'),
  t('restc', 'Spring @RestController', '@RestController\n@RequestMapping("${/api}")\npublic class ${Name}Controller {\n\n\t${}\n}'),
  t('serv', 'Spring @Service', '@Service\npublic class ${Name}Service {\n\n\t${}\n}'),
  t('logi', 'log.info', 'log.info("${message}"${});'),
  t('loge', 'log.error', 'log.error("${message}", ${e});'),
  t('tryc', 'try / catch', 'try {\n\t${}\n} catch (${Exception} e) {\n\tlog.error("${message}", e);\n\tthrow e;\n}'),
  t('testm', 'JUnit @Test', '@Test\nvoid ${should_do_something}() {\n\t// given\n\t${}\n\t// when\n\n\t// then\n\n}'),
  t('lambdah', 'AWS Lambda handler (Java)', 'public class ${Handler} implements RequestHandler<APIGatewayProxyRequestEvent, APIGatewayProxyResponseEvent> {\n\n\t@Override\n\tpublic APIGatewayProxyResponseEvent handleRequest(APIGatewayProxyRequestEvent event, Context context) {\n\t\t${}\n\t\treturn new APIGatewayProxyResponseEvent()\n\t\t\t.withStatusCode(200)\n\t\t\t.withBody("${}");\n\t}\n}'),
];

const PYTHON: Completion[] = [
  t('main', 'if __name__ == "__main__"', 'if __name__ == "__main__":\n\t${main}()'),
  t('tryc', 'try / except', 'try:\n\t${}\nexcept ${Exception} as e:\n\t${raise}'),
  t('lambdap', 'AWS Lambda handler (Python)', 'import json\n\n\ndef lambda_handler(event, context):\n\ttry:\n\t\t${}\n\t\treturn {\n\t\t\t"statusCode": 200,\n\t\t\t"body": json.dumps({"message": "${ok}"}),\n\t\t}\n\texcept Exception as e:\n\t\treturn {\n\t\t\t"statusCode": 500,\n\t\t\t"body": json.dumps({"error": str(e)}),\n\t\t}'),
];

const TS: Completion[] = [
  t('tryc', 'try / catch', 'try {\n\t${}\n} catch (${e}) {\n\tconsole.error(${e});\n\tthrow ${e};\n}'),
];

const YAML: Completion[] = [
  t('springapp', 'Spring application.yml', 'spring:\n  application:\n    name: ${app}\n  datasource:\n    url: ${jdbc:postgresql://localhost:5432/db}\n    username: ${user}\n    password: ${password}\nserver:\n  port: ${8080}\nlogging:\n  level:\n    root: ${INFO}'),
];

export const liveTemplates: Record<Lang, Completion[]> = { java: JAVA, python: PYTHON, ts: TS, yaml: YAML };

export function templateLang(path: string): Lang | null {
  const m = /\.([^./\\]+)$/.exec(path);
  switch (m?.[1].toLowerCase()) {
    case 'java': return 'java';
    case 'py': case 'pyi': return 'python';
    case 'ts': case 'tsx': case 'js': case 'jsx': case 'mjs': case 'cjs': return 'ts';
    case 'yml': case 'yaml': return 'yaml';
    default: return null;
  }
}

/** Plantillas del lenguaje de `path` (vacío si no hay). */
export function templatesFor(path: string): Completion[] {
  const l = templateLang(path);
  return l ? liveTemplates[l] : [];
}

/** Fuente de autocompletado de plantillas para el archivo `path`; convive con la del LSP. */
export function liveTemplateSource(path: string) {
  const options = templatesFor(path);
  return (ctx: CompletionContext): CompletionResult | null => {
    if (!options.length) return null;
    const word = ctx.matchBefore(/\w+/);
    if (!word && !ctx.explicit) return null;
    return { from: word ? word.from : ctx.pos, options, validFor: /^\w*$/ };
  };
}
