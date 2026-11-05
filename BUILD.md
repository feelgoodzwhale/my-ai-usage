# คู่มือการพัฒนาและ Build แอพ `my-ai-usage` (macOS)

เอกสารนี้อธิบายขั้นตอนตั้งแต่การเตรียมเครื่อง การรันโหมดพัฒนา การสั่ง Build เป็นไฟล์ `.app` และ `.dmg` ไปจนถึงการแก้ไขปัญหาที่พบบ่อยบน macOS

---

## 1. สิ่งที่ต้องมีบนเครื่อง (Prerequisites)

ตรวจสอบว่าเครื่อง Mac มีเครื่องมือพื้นฐานครบถ้วน:

1. **macOS 13.0 (Ventura) ขึ้นไป**
2. **Xcode Command Line Tools**
   ```bash
   xcode-select --install
   ```
3. **Node.js (v20+) และ npm**
   ```bash
   node -v && npm -v
   ```
4. **Rust Toolchain (`rustc` และ `cargo`)**
   หากยังไม่ได้ติดตั้ง รันคำสั่ง:
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
   source "$HOME/.cargo/env"
   ```
5. **`jq`** (สำหรับสคริปต์ `statusLine` ของ Claude และ AGY)
   ```bash
   brew install jq
   ```

---

## 2. การเตรียมโปรเจกต์ครั้งแรก (Initial Setup)

เปิด Terminal เข้าไปที่โฟลเดอร์โปรเจกต์ `my-ai-usage`:

```bash
cd my-ai-usage

# 1. ติดตั้งแพ็กเกจฝั่ง Frontend (React, Vite, Tailwind, Tauri API)
npm install

# 2. (ทางเลือก) ติดตั้ง Hook เชื่อมข้อมูล statusLine เข้าแอพ
./scripts/install-statusline-hook.sh
```

---

## 3. โหมดพัฒนาและทดสอบ (Development & Testing)

### รันแอพโหมดพัฒนา (Hot Reload)
เมื่อแก้โค้ดใน `src/` หน้าจอ UI จะอัปเดตทันที และเมื่อแก้โค้ด Rust ใน `src-tauri/` ระบบจะคอมไพล์และรีสตาร์ทแอพให้อัตโนมัติ:
```bash
npm run tauri dev
```

### ตรวจสอบค่าที่อ่านได้จากเครื่องผ่าน CLI (โดยไม่เปิด UI)
```bash
cargo run --manifest-path src-tauri/Cargo.toml -- --dump-json
```

### รัน Unit Tests ฝั่ง Rust
```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

### ทดสอบ Build เฉพาะฝั่ง Frontend (TypeScript + Vite)
```bash
npm run build
```

---

## 4. คำสั่ง Build สำหรับใช้งานจริง (Production Build)

### 4.1 สร้างครบทั้ง `.app` และ `.dmg` (มาตรฐาน)
```bash
npm run tauri build
```

### 4.2 สร้างเฉพาะไฟล์ตัวติดตั้ง `.dmg`
```bash
npm run tauri build -- --bundles dmg
```

### 4.3 สร้างเฉพาะไฟล์แอพ `.app` (เร็วกว่า ไม่ต้องรอแพ็กอิมเมจ `.dmg`)
เหมาะกับตอนแก้โค้ดเล็กน้อยแล้วอยากอัปเดตแอพในเครื่องตัวเองอย่างรวดเร็ว:
```bash
npm run tauri build -- --bundles app
```

### 4.4 สร้างแบบ Universal Binary (รองรับทั้งชิป Apple Silicon M1–M4 และ Intel)
หากต้องการส่งไฟล์ `.dmg` ให้เพื่อนที่ใช้ Mac ชิป Intel ด้วย:
```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
npm run tauri build -- --target universal-apple-darwin
```

---

## 5. ตำแหน่งไฟล์ Output หลัง Build เสร็จ

เมื่อรันคำสั่ง Build สำเร็จ ไฟล์ทั้งหมดจะอยู่ใน `src-tauri/target/release/`:

| ประเภทไฟล์ | ตำแหน่งพาธ (Path) | การใช้งาน |
| :--- | :--- | :--- |
| **ตัวติดตั้ง `.dmg`** | `src-tauri/target/release/bundle/dmg/my-ai-usage_0.2.0_aarch64.dmg` | ดับเบิลคลิกเพื่อลากแอพเข้าโฟลเดอร์ `Applications` หรือส่งต่อให้เครื่องอื่น |
| **ตัวแอพ `.app`** | `src-tauri/target/release/bundle/macos/my-ai-usage.app` | เปิดรันได้ทันที หรือคัดลอกไปวางใน `/Applications` |
| **Binary ดิบ** | `src-tauri/target/release/my-ai-usage` | รันผ่าน Terminal เช่น `./src-tauri/target/release/my-ai-usage --dump-json` |

*(หาก Build แบบ Universal Binary พาธจะเปลี่ยนจาก `target/release/...` เป็น `target/universal-apple-darwin/release/...`)*

### คำสั่งลัดจัดการไฟล์หลัง Build
```bash
# เปิดโฟลเดอร์ที่เก็บไฟล์ .dmg ใน Finder
open src-tauri/target/release/bundle/dmg

# ปิดแอพตัวเก่าที่รันอยู่ คัดลอกตัวใหม่ลง /Applications แล้วเปิดรันทันที
pkill -f my-ai-usage || true
cp -R src-tauri/target/release/bundle/macos/my-ai-usage.app /Applications/
open /Applications/my-ai-usage.app
```

---

## 6. การเปลี่ยนไอคอนแอพและการอัปเดตเวอร์ชัน

### การเปลี่ยนไอคอนแอพใหม่
วางไฟล์ต้นฉบับขนาด `1024x1024` (`.svg` หรือ `.png`) เช่น `app-icon.svg` แล้วสั่งสร้างไอคอนทุกขนาดอัตโนมัติ:
```bash
npx tauri icon app-icon.svg
```

### การปรับเลขเวอร์ชันก่อน Build รอบใหม่
เมื่อต้องการออกเวอร์ชันใหม่ (เช่นเปลี่ยนจาก `0.2.0` เป็น `0.3.0`) ให้อัปเดตเลขเวอร์ชันให้ตรงกันใน 3 ไฟล์:
1. `package.json` (`"version": "0.3.0"`)
2. `src-tauri/tauri.conf.json` (`"version": "0.3.0"`)
3. `src-tauri/Cargo.toml` (`version = "0.3.0"`)

ไฟล์ `.dmg` ที่ได้จะเปลี่ยนชื่อตามเวอร์ชันอัตโนมัติ เช่น `my-ai-usage_0.3.0_aarch64.dmg`

---

## 7. การแก้ไขปัญหาที่พบบ่อย (Troubleshooting)

* **Error: `failed to run command cargo metadata ... No such file or directory`**
  * เกิดจาก Terminal ยังไม่โหลดพาธ `~/.cargo/bin` แก้ไขโดยรัน:
    ```bash
    source "$HOME/.cargo/env"
    ```
    *(ในโปรเจกต์นี้ได้ตั้งค่าแนบ `$HOME/.cargo/bin` ไว้ใน `package.json` ให้แล้ว หากใช้ผ่าน `npm run tauri ...` จะทำงานได้ทันที)*

* **Build `.dmg` ล้มเหลวในขั้นตอน `bundle_dmg.sh` หรือ `hdiutil`**
  * มักเกิดจากเปิด Mount ไฟล์ `.dmg` ตัวเก่าค้างไว้บน Desktop/Finder ให้กด **Eject** ไดรฟ์ `my-ai-usage` ใน Finder ออกก่อน แล้วสั่ง `npm run tauri build` ใหม่

* **นำไฟล์ `.dmg` ไปติดตั้งในเครื่อง Mac เครื่องอื่นแล้วขึ้นเตือน *"my-ai-usage is damaged and can't be opened"* หรือ *"unidentified developer"***
  * เนื่องจากเป็นการ Build แบบ Local โดยยังไม่ได้ผูก Apple Developer Certificate ($99/ปี) เมื่อลากลง `/Applications` ในเครื่องอื่นแล้ว ให้รันคำสั่งปลด Quarantine ของ macOS ครั้งเดียว:
    ```bash
    xattr -cr /Applications/my-ai-usage.app
    ```

* **ต้องการล้างไฟล์แคช Build ทั้งหมดเพื่อ Build ใหม่ตั้งแต่ต้น (Clean Build)**
  ```bash
  rm -rf dist
  cargo clean --manifest-path src-tauri/Cargo.toml
  npm run tauri build
  ```
