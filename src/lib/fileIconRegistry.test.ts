import { describe, expect, it } from 'vitest';
import { fileIconKey, folderIconKey, folderIconName, isExcludedFolderName, stereotypeOverlay, symbolLabel } from './fileIconRegistry';
import type { SymbolInfo } from './types';

const sym = (kind: SymbolInfo['kind'], stereotype: SymbolInfo['stereotype'] = null): SymbolInfo => ({ kind, stereotype, public: true });

describe('fileIconKey', () => {
  it('archivos por nombre', () => {
    expect(fileIconKey('pom.xml', 'xml')).toBe('pom');
    expect(fileIconKey('build.gradle.kts', 'kts')).toBe('gradle');
    expect(fileIconKey('settings.gradle', 'gradle')).toBe('gradle');
    expect(fileIconKey('Dockerfile', null)).toBe('docker');
    expect(fileIconKey('docker-compose.prod.yml', 'yml')).toBe('docker');
    expect(fileIconKey('.gitignore', null)).toBe('git');
    expect(fileIconKey('.dockerignore', null)).toBe('dockerignore');
    expect(fileIconKey('Makefile', null)).toBe('makefile');
    expect(fileIconKey('LICENSE', null)).toBe('license');
    expect(fileIconKey('caesar.json', 'json')).toBe('caesar');
  });
  it('JVM / Spring', () => {
    expect(fileIconKey('mvnw', null)).toBe('mvnw');
    expect(fileIconKey('gradlew', null)).toBe('mvnw');
    expect(fileIconKey('mvnw.cmd', null)).toBe('mvnwCmd');
    expect(fileIconKey('HELP.md', 'md')).toBe('helpMd');
    expect(fileIconKey('FinanceApplication.java', 'java')).toBe('springApp');
    expect(fileIconKey('FinanceApplication.java', 'java', sym('class', 'springBootApp'))).toBe('sym:class');
    expect(fileIconKey('app.jar', 'jar')).toBe('jar');
    expect(fileIconKey('x.kts', 'kts')).toBe('kotlin');
    expect(fileIconKey('messages.properties', 'properties')).toBe('properties');
  });
  it('Python', () => {
    expect(fileIconKey('pyproject.toml', 'toml')).toBe('pyproject');
    expect(fileIconKey('requirements.txt', 'txt')).toBe('requirements');
    expect(fileIconKey('Pipfile', null)).toBe('pipfile');
    expect(fileIconKey('poetry.lock', 'lock')).toBe('poetry');
    expect(fileIconKey('uv.lock', 'lock')).toBe('uv');
    expect(fileIconKey('manage.py', 'py')).toBe('djangoManage');
    expect(fileIconKey('a.py', 'py')).toBe('python');
    expect(fileIconKey('a.pyi', 'pyi')).toBe('python');
    expect(fileIconKey('a.ipynb', 'ipynb')).toBe('notebook');
  });
  it('Node / TS / Web', () => {
    expect(fileIconKey('package.json', 'json')).toBe('packageJson');
    expect(fileIconKey('tsconfig.json', 'json')).toBe('tsconfig');
    expect(fileIconKey('vite.config.ts', 'ts')).toBe('vite');
    expect(fileIconKey('next.config.mjs', 'mjs')).toBe('next');
    expect(fileIconKey('tailwind.config.js', 'js')).toBe('tailwind');
    expect(fileIconKey('pnpm-lock.yaml', 'yaml')).toBe('pnpm');
    expect(fileIconKey('yarn.lock', 'lock')).toBe('yarn');
    expect(fileIconKey('bun.lockb', 'lockb')).toBe('bun');
    expect(fileIconKey('a.mts', 'mts')).toBe('typescript');
    expect(fileIconKey('a.tsx', 'tsx')).toBe('reactTs');
    expect(fileIconKey('a.jsx', 'jsx')).toBe('react');
    expect(fileIconKey('a.mjs', 'mjs')).toBe('javascript');
    expect(fileIconKey('a.css', 'css')).toBe('css');
    expect(fileIconKey('a.scss', 'scss')).toBe('scss');
    expect(fileIconKey('a.svg', 'svg')).toBe('svg');
  });
  it('Rust / Go / C', () => {
    expect(fileIconKey('Cargo.toml', 'toml')).toBe('cargo');
    expect(fileIconKey('Cargo.lock', 'lock')).toBe('cargo');
    expect(fileIconKey('lib.rs', 'rs')).toBe('rust');
    expect(fileIconKey('go.mod', 'mod')).toBe('gomod');
    expect(fileIconKey('go.sum', 'sum')).toBe('gomod');
    expect(fileIconKey('main.go', 'go')).toBe('go');
    expect(fileIconKey('a.c', 'c')).toBe('c');
    expect(fileIconKey('a.cpp', 'cpp')).toBe('cpp');
    expect(fileIconKey('a.h', 'h')).toBe('h');
  });
  it('config y docs', () => {
    expect(fileIconKey('a.toml', 'toml')).toBe('toml');
    expect(fileIconKey('a.ini', 'ini')).toBe('ini');
    expect(fileIconKey('a.xml', 'xml')).toBe('xml');
    expect(fileIconKey('a.graphql', 'graphql')).toBe('graphql');
    expect(fileIconKey('a.proto', 'proto')).toBe('proto');
    expect(fileIconKey('a.sh', 'sh')).toBe('shell');
    expect(fileIconKey('a.zsh', 'zsh')).toBe('shell');
    expect(fileIconKey('a.txt', 'txt')).toBe('text');
    expect(fileIconKey('README.md', 'md')).toBe('readme');
    expect(fileIconKey('notes.md', 'md')).toBe('markdown');
  });
  it('configuración Spring y yml/properties', () => {
    expect(fileIconKey('application.yml', 'yml')).toBe('springConfig');
    expect(fileIconKey('application-dev.properties', 'properties')).toBe('springConfig');
    expect(fileIconKey('other.yml', 'yml')).toBe('yaml');
    expect(fileIconKey('messages.properties', 'properties')).toBe('properties');
  });
  it('.env y variantes', () => {
    expect(fileIconKey('.env.local', 'local')).toBe('env');
    expect(fileIconKey('.env', null)).toBe('env');
  });
  it('tests no JVM', () => {
    expect(fileIconKey('x.test.ts', 'ts')).toBe('testFile');
    expect(fileIconKey('foo.spec.js', 'js')).toBe('testFile');
    expect(fileIconKey('test_api.py', 'py')).toBe('testFile');
    expect(fileIconKey('x.ts', 'ts')).toBe('typescript');
  });
  it('tipos comunes', () => {
    expect(fileIconKey('index.html', 'html')).toBe('html');
    expect(fileIconKey('a.json', 'json')).toBe('json');
    expect(fileIconKey('q.sql', 'sql')).toBe('sql');
    expect(fileIconKey('logo.png', 'png')).toBe('image');
    expect(fileIconKey('weird', null)).toBe('generic');
  });
  it('java y kotlin con/sin símbolo', () => {
    expect(fileIconKey('UserService.java', 'java')).toBe('java');
    expect(fileIconKey('UserService.java', 'java', sym('class', 'service'))).toBe('sym:class');
    expect(fileIconKey('Shape.java', 'java', sym('abstractClass'))).toBe('sym:abstractClass');
    expect(fileIconKey('Money.kt', 'kt')).toBe('kotlin');
    expect(fileIconKey('Money.kt', 'kt', sym('dataClass'))).toBe('sym:dataClass');
    expect(fileIconKey('X.java', 'java', sym('unknown'))).toBe('java');
    // un test Java no cae en 'testFile'
    expect(fileIconKey('FooTest.java', 'java')).toBe('java');
  });
});

describe('overlays y etiquetas', () => {
  it('overlay por estereotipo', () => {
    expect(stereotypeOverlay('springBootApp')).toBe('springRun');
    expect(stereotypeOverlay('main')).toBe('run');
    expect(stereotypeOverlay('controller')).toBe('spring');
    expect(stereotypeOverlay('repository')).toBe('spring');
    expect(stereotypeOverlay('test')).toBe('test');
    expect(stereotypeOverlay(null)).toBeNull();
  });
  it('tooltip', () => {
    expect(symbolLabel(sym('class', 'controller'))).toBe('Clase · @RestController');
    expect(symbolLabel(sym('interface'))).toBe('Interfaz');
    expect(symbolLabel(undefined)).toBe('');
  });
});

describe('folderIconKey', () => {
  it('el rol gana', () => {
    expect(folderIconKey({ cls: 'plain', role: 'sourceRoot' })).toBe('sourceRoot');
    expect(folderIconKey({ cls: 'source', role: 'package' })).toBe('package');
    expect(folderIconKey({ cls: 'plain', role: 'generatedRoot' })).toBe('generatedRoot');
  });
  it('sin rol usa la clase', () => {
    expect(folderIconKey({ cls: 'test' })).toBe('testRoot');
    expect(folderIconKey({ cls: 'excluded' })).toBe('excluded');
    expect(folderIconKey({ cls: 'plain' })).toBe('plain');
    expect(folderIconKey(undefined)).toBe('plain');
  });
});

describe('folderIconName', () => {
  it('el nombre gana al rol', () => {
    expect(folderIconName('controller', { cls: 'package', role: 'package' })).toBe('controllers');
    expect(folderIconName('Services', { cls: 'plain' })).toBe('services');
    expect(folderIconName('repository', undefined)).toBe('repositories');
    expect(folderIconName('repositories', undefined)).toBe('repositories');
    expect(folderIconName('models', undefined)).toBe('models');
    expect(folderIconName('src', { cls: 'source', role: 'sourceRoot' })).toBe('src');
    expect(folderIconName('__tests__', undefined)).toBe('tests');
    expect(folderIconName('.github', undefined)).toBe('github');
    expect(folderIconName('colecciones', undefined)).toBe('collections');
  });
  it('sin nombre conocido cae al rol/clase', () => {
    expect(folderIconName('com', { cls: 'package', role: 'package' })).toBe('package');
    expect(folderIconName('foo', { cls: 'test' })).toBe('testRoot');
    expect(folderIconName('foo', { cls: 'module' })).toBe('module');
    expect(folderIconName('foo', undefined)).toBe('plain');
  });
  it('excluidas', () => {
    expect(isExcludedFolderName(folderIconName('node_modules', undefined))).toBe(true);
    expect(isExcludedFolderName(folderIconName('target', { cls: 'excluded', role: 'excluded' }))).toBe(true);
    expect(isExcludedFolderName(folderIconName('whatever', { cls: 'excluded', role: 'excluded' }))).toBe(true);
    expect(isExcludedFolderName(folderIconName('src', undefined))).toBe(false);
  });
});
