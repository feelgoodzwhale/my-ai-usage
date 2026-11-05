# my-ai-usage

macOS Menu Bar utility สร้างด้วย **Rust (Tauri v2)** + **React + TypeScript + Tailwind CSS** สำหรับแสดงข้อมูลการใช้งาน **Claude** และ **AGY (Google Antigravity)** แบบเรียลไทม์จากข้อมูล Local บนเครื่อง

## ฟีเจอร์หลัก
- **2 ไอคอนแยกอิสระบน macOS Menu Bar** (`Claude` และ `AGY`) พร้อมตัวเลข `%` โควต้ารอบ 5 ชั่วโมง และสีแจ้งเตือนอัตโนมัติ (`<75%` สีกลืนตามระบบ, `75-89%` สีส้ม, `>=90%` สีแดง)
- **Mini Hover Card ทันทีเมื่อชี้เมาส์:** แสดงหลอด `5-Hour Limit`, `Weekly Limit (7d)` และเวลานับถอยหลัง `Reset time`
- **Dashboard Popover เมื่อคลิก:**
  - แยกดูสถิติตามช่วงเวลา `รอบ 5h`, `วันนี้`, `7 วัน`, `30 วัน`
  - **Token Breakdown 4 ประเภท:** `Input`, `Output`, `Cache Read`, `Cache Write`
  - **กราฟแนวโน้มการใช้งาน:** รายชั่วโมง (24 ชม.) และรายวัน (7–30 วัน)
  - **Model Breakdown:** สัดส่วนการใช้งานแยกตามรุ่นโมเดล
  - **Settings:** ปรับเพดาน Token ของ AGY, เลือกสูตรคำนวณ Cache, เปิด/ปิด macOS Notification (`80%` / `95%` / `Reset`) และ `Launch at Login`

## คำสั่งใช้งาน

```bash
# โหมดพัฒนา (Hot Reload)
npm run tauri dev

# ตรวจสอบข้อมูลที่อ่านได้จากเครื่องแบบ JSON ผ่าน CLI
cargo run --manifest-path src-tauri/Cargo.toml -- --dump-json

# สร้างไฟล์แอพ macOS (.app / .dmg)
npm run tauri build
```

สำหรับรายละเอียดขั้นตอนการเตรียมเครื่อง คำสั่งแยก Build เฉพาะ `.dmg` / `.app` / Universal Binary ตำแหน่งไฟล์ Output และการแก้ปัญหาทั้งหมด ดูได้ที่ **[BUILD.md](./BUILD.md)**
