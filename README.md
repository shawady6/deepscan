# DeepScan (Rust + Tauri)

برنامج ديسك توب لفحص الجهاز: مواصفات، تخزين، برامج، شبكة، أمان، كارت مشاركة وتقرير كامل.
الـ backend بـ Rust، والواجهة HTML/JS جوه WebView2 (موجود في ويندوز 10/11). الناتج exe واحد صغير.

## بناء الـ exe
على ويندوز بعد تثبيت Rust: شغّل `build.bat` ← الناتج `target\release\DeepScan.exe`.
أو ارفع المجلد على GitHub وشغّل workflow "Build exe" ونزّل الـ artifact.

## هيكل المشروع
- `core/` : كود الفحص (بدون واجهة). `cargo test -p deepscan-core` بيشتغل على أي نظام (بيرجع بيانات تجريبية لو مش ويندوز).
- `src-tauri/` : غلاف التطبيق (الأوامر اللي الواجهة بتناديها + طلب صلاحيات Admin).
- `ui/` : الواجهة.

## ملاحظات
- الـ exe بيطلب Admin (لازم لفحص BitLocker وTPM وSMB1).
- معلومات الـ Wi-Fi بتتقرأ صح على ويندوز إنجليزي فقط حاليًا.
- حرارة المعالج مش دقيقة (محتاجة LibreHardwareMonitor).
- الإعدادات وتاريخ السرعة في `%APPDATA%\DeepScan`.

## التحديثات من داخل التطبيق
- التطبيق بيفحص آخر Release على GitHub (`shawady6/deepscan`) عند التشغيل وبيظهر شريط "Update now"، وفي زرار "Check for updates" في الشريط الجانبي وتبويب Reports.
- لازم الريبو يكون **Public** عشان الفحص يشتغل بدون توكن.
- لنشر نسخة جديدة: غيّر `version` في `core/Cargo.toml` و`src-tauri/Cargo.toml` و`src-tauri/tauri.conf.json`، ثم `git tag v1.1.0 && git push origin v1.1.0`. الـ workflow "Release" بيبني ويرفع `DeepScan.exe` + `DeepScan.exe.sha256`.
- التطبيق بيرفض التحديث لو الملف مش من نفس الريبو أو الـ checksum مش مطابق.
