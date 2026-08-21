# Prompt redesign UI — AnakMobil (untuk Claude Design)

Salin blok di bawah ke Claude Design. Design system `AnakMobil Design System` dan
repo `oksasatya/anak-mobil` sudah terpasang, jadi prompt ini menyuruhnya membaca
kode yang ada, bukan menebak.

---

## PROMPT UTAMA

```
Redesign seluruh UI aplikasi mobile AnakMobil.id. Aplikasinya sudah jalan dan
semua layar di bawah sudah ada kodenya di repo yang terpasang — baca dulu
sebelum mendesain, jangan mulai dari nol. Yang saya minta adalah tampilan yang
lebih bersih dan modern, BUKAN sistem desain baru.

## Produk

Garasi digital untuk pemilik mobil di Indonesia. Orang mencatat servis dan
modifikasi mobilnya, melihat riwayat perawatan, dan (nanti) bertanya ke sesama
pemilik. Penggunanya campuran: penggemar otomotif yang detail, dan pemilik biasa
yang cuma ingin tahu kapan servis berikutnya. Dipakai sambil berdiri di bengkel,
sering di bawah sinar matahari.

## Design system — SUDAH ADA, jangan diganti

Semua nilai di bawah sudah dipakai dan sudah lolos kontras. Pakai ini; jangan
memperkenalkan hex, ukuran font, spasi, atau radius baru.

WARNA
- Ground (grafit, ~85% layar): #0F141A · #151B22 · #1D232A · #2B323B · #3C4550
- Aksen oranye: #ED491C (utama) · #C93413 · #DC3E17 · #F45C32 · #FF805E
- Permukaan gelap: surface #151A20 · surfaceSubtle #202730 · surfaceRaised #1B2128
- Teks gelap: primary #F5F7F8 · secondary #AAB2BA · border #29313A · borderStrong #3A434E
- Permukaan terang: surface #FFFFFF · surfaceSubtle #F1F3F5
- Teks terang: primary #171C22 · secondary #5D6670 · border #E3E6E9
- Status: success #168A52 · warning #D58A00 · danger #D63B3B · info #2678D9

ATURAN KERAS SOAL ORANYE: maksimal ~10% layar, dan HANYA untuk aksi utama,
status terpilih, sorotan AI, atau penanda yang benar-benar penting. Oranye BUKAN
warna status — success/warning/danger/info punya nilainya sendiri. Satu tombol
oranye per layar. Sisanya grafit.

TIPOGRAFI — GANTI KE PLUS JAKARTA SANS (sebelumnya Inter)

Pakai **Plus Jakarta Sans** di semua layar. Ini satu-satunya bagian design
system yang memang saya minta berubah. Alasannya bukan selera: huruf ini dibuat
untuk identitas kota Jakarta oleh Tokotype, jadi untuk produk yang Indonesia-first
dia membawa karakter lokal yang Inter tidak punya — sementara bentuk hurufnya
tetap netral dan geometris, jadi angka spesifikasi mobil tetap terbaca cepat.

Skalanya TIDAK berubah, cuma huruf yang ganti:
- display 700 32/38 · h1 700 28/34 · h2 700 24/30 · h3 650 20/26
- title 650 18/24 · body-lg 400 16/24 · body 400 14/21
- label 600 13/18 · caption 400 12/17 · micro 500 11/14

Catatan penting soal berat huruf: aplikasi sekarang memakai empat potongan
statis (400/500/600/700), dan berat **650** turun ke 600 karena tidak ada
potongan statisnya. Plus Jakarta Sans punya **variable cut 200–800**, jadi 650
bisa benar-benar dipakai kalau kita muat versi variabelnya. Rancang dengan 650
yang asli, dan sebut di catatan kalau perbedaan 600 vs 650 itu penting di layar
tertentu.

Angka (kilometer, rupiah, tanggal) pakai tabular figures — Plus Jakarta Sans
mendukungnya lewat fitur `tnum`.

SPASI: 4 · 8 · 12 · 16 · 20 · 24 · 32 · 40 · 48 · 64 · 80 · 96
RADIUS: 6 · 8 · 12 · 16 · 20 · 28 · 999
Padding halaman 16. Target sentuh minimal 44pt. Input minimal 52pt.

ELEVASI: border dulu, baru bayangan. Permukaan biasa = border 1px, tanpa
bayangan. Bayangan hanya untuk yang benar-benar mengambang (bottom sheet,
dropdown).

MOTION: 150ms mikro · 210ms standar · 280ms sheet. Pendek dan fungsional.
Tidak ada efek oranye yang menyala terus-menerus.

DUA TEMA: gelap dan terang, dua-duanya harus dirancang, bukan salah satunya
dibalik. Ground grafitnya harus tembus di belakang tab bar.

## Komponen yang sudah ada — pakai ulang, jangan bikin kembar

AmButton (primary/accent/secondary/ghost/destructive; sm 36 / md 44 / lg 52)
AmTextField · AmSelect · AmCard · AmChip · AmBadge · AmAvatar
AmBottomSheet (semua pilihan pakai ini — TIDAK ADA picker native, TIDAK ADA Alert)
AmGround (gradien grafit) · AmMaterial · AmEmptyState · AmErrorState
AmSkeleton · AmToast · AmBrandLockup (logo + wordmark "AnakMobil.id")

Kalau sebuah layar butuh sesuatu yang belum ada, katakan komponen baru apa yang
perlu ditambahkan ke design system — jangan gambar satu kali pakai.

## Layar yang harus didesain ulang — semuanya

1. LAUNCH — logo + wordmark di atas ground grafit, wordmark muncul beranimasi.
2. MASUK — email, kata sandi, tombol utama, tautan ke daftar.
3. DAFTAR — email, username (dengan cek ketersediaan langsung), kata sandi
   (meter kekuatan), checkbox persetujuan syarat, tombol utama.
4. BERANDA — kartu mobil aktif (nama, tahun, warna, kilometer, ringkasan
   servis), pemilih mobil aktif kalau punya lebih dari satu.
5. GARASI — daftar semua mobil (sekarang masih empty state).
6. JELAJAH — belum ada isinya (empty state).
7. KOMUNITAS — belum ada isinya (empty state).
8. PROFIL — avatar/inisial, nama, @username, email, tombol keluar.
9. TAB BAR — lima tab: Beranda · Garasi · Jelajah · Komunitas · Profil.
10. SHEET TAMBAH — pilih aksi (modifikasi/servis/problem/foto) dengan mobil
    aktif sudah terisi dan bisa diganti.
11. ONBOARDING — tambah mobil pertama (belum dibangun, desain dari nol).

Untuk SETIAP layar, rancang seluruh keadaannya, jangan cuma yang ideal:
memuat (skeleton) · kosong · error · offline · sedang mengirim · sukses.
Empty state dan error state itu layar utama di produk ini, bukan tempelan.

## Aturan produk yang TIDAK BOLEH dilanggar desain

Ini keputusan produk, bukan preferensi. Desain yang melanggarnya tidak bisa
dipakai walaupun bagus.

1. TIDAK ADA DATA PALSU. Jangan gambar jumlah anggota, testimoni, aktivitas,
   rating, atau mobil contoh yang tidak ada. Aplikasinya diluncurkan kosong dan
   mengatakannya terus terang. Empty state adalah ajakan bertindak, bukan
   permintaan maaf.

2. TIDAK ADA KONTROL YANG TUJUANNYA TIDAK ADA. Jangan gambar tombol "Edit
   profil", "Notifikasi", "Bantuan", atau menu pengaturan kalau layarnya belum
   dibangun. Lebih baik layarnya lebih sepi daripada punya jalan buntu.

3. DATA PRIBADI KENDARAAN TIDAK PERNAH TAMPIL DI DAFTAR. Plat nomor, nomor
   rangka (VIN), dan harga beli tidak dikirim server ke layar daftar. Jangan
   gambar di kartu mobil.

4. BIAYA NULL DIHILANGKAN, BUKAN DITULIS "Rp 0". Mobil tanpa riwayat servis
   tidak punya total — barisnya hilang. "Rp 0" itu bohong: artinya ada yang
   mencatat pengeluaran nol, padahal tidak ada yang mencatat apa pun.

5. ERROR MASUK SERAGAM. Email tidak dikenal dan kata sandi salah harus
   menghasilkan tampilan yang persis sama. Tidak ada petunjuk, tidak ada
   "mau daftar?", tidak ada perbedaan penempatan. Ini keamanan, bukan gaya
   bahasa.

6. SEMUA TEKS PRODUK BAHASA INDONESIA, dan pakai istilah yang benar-benar
   dipakai orang bengkel dan penggemar mobil — bukan terjemahan harfiah.

7. AKSESIBILITAS: kontras minimal 4.5:1 untuk teks biasa, target sentuh 44pt,
   teks harus tetap terbaca dan mengalir saat ukuran font sistem diperbesar
   (jangan mendesain yang pecah kalau teks membesar).

## Bentuk data asli — supaya mockup tidak mengarang

Kartu mobil berisi: nama ("Toyota Avanza Veloz 2020"), nickname ("Si Putih"),
tahun (2019), warna ("Putih"), kilometer (146120 → "146.120 km"), dan ringkasan:
jumlah servis (2), total biaya ("4200000.00" → "Rp 4.200.000"), tanggal servis
terakhir ("2026-08-12" → "12 Agu 2026"), jumlah terlambat (0), jumlah segera (0).
Rupiah tanpa sen. Badge terlambat/segera hanya muncul kalau angkanya di atas nol.

## Arah visual yang saya mau

Bersih dan modern. Grafit gelap yang terasa mahal, bukan gelap yang suram. Ruang
kosong yang disengaja. Hierarki dari ukuran dan jarak, bukan dari garis dan
kotak di mana-mana. Data mobil harus terbaca sekali lihat sambil berdiri di
bengkel.

Yang saya TIDAK mau: kartu yang bertumpuk-tumpuk dengan border tipis di
mana-mana, gradien ungu-biru, glassmorphism di segala permukaan, ikon emoji,
atau layar yang penuh tapi tidak mengatakan apa-apa.

## Yang saya harapkan sebagai hasil

- Setiap layar di atas, dalam tema gelap DAN terang.
- Semua keadaan (memuat/kosong/error/mengirim), bukan cuma keadaan ideal.
- Catatan singkat per layar: apa yang berubah dari yang sekarang, dan kenapa.
- Daftar komponen baru yang perlu masuk design system, kalau ada.
- Daftar aset gambar yang dibutuhkan (ilustrasi empty state, dsb) — cukup
  jelaskan apa yang dibutuhkan, jangan digambar.
```

---

## PROMPT ASET (untuk ChatGPT / generator gambar)

Baru dipakai **setelah** Claude Design bilang aset apa yang benar-benar
dibutuhkan. Jangan bikin aset sebelum tahu yang mana yang perlu.

### Aturan yang berlaku untuk semua aset

- Palet dikunci: grafit `#0F141A`–`#3C4550`, aksen oranye `#ED491C`. **Tidak
  boleh ada biru.**
- Latar **transparan** (PNG), karena ground-nya gradien dan harus tembus.
- Format persegi atau rasio yang disebut, resolusi minimal 1024px.
- **Tidak ada teks di dalam gambar** — semua teks dirender aplikasi supaya ikut
  tema dan ukuran font pengguna.
- **Jangan bikin foto mobil yang terlihat asli.** Foto AI yang dipasang sebagai
  mobil sungguhan melanggar aturan "tidak ada data palsu". Ilustrasi yang jelas
  ilustrasi — aman. Foto realistis — tidak.

### Contoh prompt ilustrasi empty state

```
Flat vector illustration, transparent background, 1024x1024.
Subject: an empty car garage interior, seen straight on, minimal and geometric.
Style: clean flat vector, thin consistent line weight, no gradients, no shadows,
no text, no people.
Colour: only these — dark graphite #1D232A and #2B323B for structure, a single
warm orange #ED491C accent used sparingly on ONE element (about 10% of the
image), light grey #AAB2BA for secondary lines.
Mood: calm and inviting, not sad or apologetic. It should read as "ruang untuk
mobilmu", a space waiting to be filled — never as an error.
Do not include: any text, any logo, any real car brand, any photorealism,
any blue, any purple gradient.
```

Ganti `Subject:` sesuai kebutuhan:
- Jelajah kosong → *a magnifying glass over a simple road map, geometric*
- Komunitas kosong → *three simple abstract figures around a car silhouette*
- Onboarding mobil pertama → *a simple car outline with a plus sign, front view*
- Error/offline → *a disconnected plug or a road with a gap, geometric*

### Ikon aplikasi (kalau mau diganti)

Ikon sekarang **turunan** — hasil script dari `favicon-dark.png`, dipotong
kotak dan tile-nya dibuang, di-downscale dari ~1117px. Untuk App Store idealnya
di-export dari file vektor aslinya:

```
1024x1024 PNG, NO transparency, NO rounded corners (iOS masks its own).
The AnakMobil mark — a graphite garage/house outline containing the letters
"AM", with an orange road curving through it — centred with even margin.
Background: solid graphite #1D232A or a subtle vertical gradient from #22262F
to #171920. Orange #ED491C on the road only.
No text, no wordmark, no border, no drop shadow.
```

---

## Catatan sebelum mulai

Redesain ini **hanya visual**. Aturan produk di bagian "TIDAK BOLEH dilanggar"
bukan gaya — masing-masing sudah pernah jadi cacat nyata di kode ini, dan
tercatat di `docs/superpowers/plans/*.md`. Desain yang melanggarnya akan ditolak
di implementasi, jadi lebih murah dipatuhi dari awal.

Kalau hasil desainnya menuntut token baru (warna, ukuran, spasi), itu keputusan
design system dan harus lewat `docs/design.md` + `packages/tokens` dulu — bukan
ditulis langsung di layar.

### Ganti font itu perubahan kode, bukan cuma perubahan mockup

Supaya tidak kaget di implementasi — Plus Jakarta Sans menyentuh tiga tempat:

1. `apps/mobile/src/theme/fonts.ts` — sekarang mengimpor empat potongan statis
   Inter lewat **deep import** (`@expo-google-fonts/inter/400Regular`), bukan
   barrel-nya. Itu disengaja: barrel-nya menarik 18 potongan (~6,1 MB) padahal
   cuma empat yang dipakai (~1,35 MB), dan Metro tidak membuang named export
   yang tidak terpakai dari sebuah `require()` barrel. **Pertahankan pola deep
   import itu** waktu ganti ke `@expo-google-fonts/plus-jakarta-sans`.
2. `packages/tokens/src/tokens.js` — komentar di skala tipografi menyebut Inter
   dan menjelaskan kenapa 650/750 butuh variable cut. Harus ikut diperbarui.
3. `docs/design.md` §11 — mencatat substitusi 650 → 600. Kalau kita jadi memuat
   variable cut-nya, catatan itu berubah dari "substitusi" jadi "tidak perlu
   lagi", dan itu perbaikan yang layak ditulis.

Keputusan yang masih terbuka dan sebaiknya diambil sadar: **potongan statis atau
variable?** Statis lebih kecil dan sama seperti sekarang, tapi 650 tetap jatuh
ke 600. Variable memberi 650 yang asli dan semua berat di antaranya, dengan satu
file yang biasanya lebih besar dari empat statis. Untuk aplikasi yang skalanya
memang memakai 650 di dua tempat (h3 dan title), variable kelihatannya lebih
tepat — tapi ukurannya perlu diukur dulu, bukan diasumsikan.
