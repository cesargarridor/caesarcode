import { describe, expect, it } from 'vitest';
import {
  checkArtifact, checkGoModule, checkPackage, checkPort, checkProjectName, checkServiceName,
  deriveArtifact, derivePackage, nextPort, pkgSegment, previewPath,
} from './projectValidate';

describe('projectValidate', () => {
  it('nombres de proyecto', () => {
    for (const ok of ['app', 'My-App_1.2', 'a']) expect(checkProjectName(ok)).toBeNull();
    for (const bad of ['', '-x', '../evil', 'a..b', 'CON', 'con.txt', 'x.', '1abc', 'a b', 'a'.repeat(65)]) {
      expect(checkProjectName(bad), bad).not.toBeNull();
    }
  });

  it('microservicios kebab', () => {
    expect(checkServiceName('orders-service')).toBeNull();
    expect(checkServiceName('Orders')).not.toBeNull();
    expect(checkServiceName('a--b')).not.toBeNull();
  });

  it('paquetes y artifactId', () => {
    expect(checkPackage('com.example.app')).toBeNull();
    for (const bad of ['com.class.x', 'com..x', '-x', 'Com.x', 'com.new']) expect(checkPackage(bad), bad).not.toBeNull();
    expect(checkArtifact('demo-app')).toBeNull();
    expect(checkArtifact('Demo')).not.toBeNull();
  });

  it('go, puertos', () => {
    expect(checkGoModule('github.com/me/app')).toBeNull();
    expect(checkGoModule('../x')).not.toBeNull();
    expect(checkPort(8080)).toBeNull();
    expect(checkPort(80)).not.toBeNull();
    expect(checkPort(70000)).not.toBeNull();
  });

  it('derivaciones', () => {
    expect(deriveArtifact('My App')).toBe('my-app');
    expect(deriveArtifact('123')).toBe('app-123');
    expect(derivePackage('com.example', 'orders-service')).toBe('com.example.ordersservice');
    expect(derivePackage('com.example', 'new')).toBe('com.example.new_');
    expect(checkPackage(derivePackage('com.example', 'new'))).toBeNull();
    expect(pkgSegment('api-gateway')).toBe('apigateway');
  });

  it('puertos libres y ruta', () => {
    expect(nextPort([8081, 8082])).toBe(8083);
    expect(nextPort([])).toBe(8081);
    expect(previewPath('/home/u/', 'proj')).toBe('/home/u/proj');
    expect(previewPath('C:\\Users\\u', 'proj')).toBe('C:\\Users\\u\\proj');
  });
});
