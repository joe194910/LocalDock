import { access, readFile, readdir } from "node:fs/promises";
import { resolve } from "node:path";

const root = resolve(process.cwd());
const groups = ["app-packages", "tool-packages"];
let count = 0;

for (const group of groups) {
  const names = await readdir(resolve(root, group));
  for (const name of names) {
    const manifestPath = resolve(root, group, name, "manifest.json");
    const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
    if (manifest.schemaVersion !== 1 || !manifest.id || !manifest.kind || !manifest.version) {
      throw new Error(`应用包清单无效：${manifestPath}`);
    }
    if (manifest.version === "latest") {
      throw new Error(`应用包禁止使用 latest：${manifestPath}`);
    }
    if (manifest.kind === "container_app" && manifest.capabilities?.includes("compose")) {
      if (!manifest.imageVariable || !manifest.composeTemplate) {
        throw new Error(`Compose 应用包缺少镜像变量或模板声明：${manifestPath}`);
      }
      await access(resolve(root, group, name, manifest.composeTemplate));
    }
    if (manifest.id === "jenkins") {
      if (manifest.imageRepository !== "jenkins/jenkins" || !manifest.imageTag || manifest.imageTag.includes("latest")) {
        throw new Error(`Jenkins 应用包必须使用官方仓库和固定镜像标签：${manifestPath}`);
      }
    }
    count += 1;
  }
}

console.log(`已验证 ${count} 个声明式应用包。`);
