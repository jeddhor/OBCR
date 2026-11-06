// OBCR Setup: installation logic shared by the wizard and unattended mode.
// SPDX-License-Identifier: GPL-3.0-or-later
#include "setup.h"

#include <QCoreApplication>
#include <QDir>
#include <QDirIterator>
#include <QFile>
#include <QFileInfo>
#include <QPainter>
#include <QPainterPath>
#include <QProcess>
#include <QSaveFile>
#include <QSettings>
#include <QStandardPaths>
#include <QStorageInfo>
#include <QTemporaryDir>
#include <QTextStream>

#include <algorithm>
#include <memory>

namespace setup {

namespace {

constexpr auto kManifest = "install-manifest.txt";
// Written by the packaging scripts next to the Setup executable: the
// runtime files (relative paths) the uninstaller copy needs.
constexpr auto kRuntimeList = "setup-runtime.txt";
constexpr auto kUninstallDir = "uninstall";
constexpr auto kDataDir = "data";
constexpr auto kIconFile = "obcr.png";
constexpr auto kSettingsOrg = "OBCR";
constexpr auto kSettingsApp = "Setup";
#ifdef Q_OS_WIN
constexpr auto kUninstallKey =
    "HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\OBCR";
#endif

// What an installation created, in order, so it can be removed exactly.
struct Manifest {
    QStringList files;  // F: files (inside or outside the install folder)
    QStringList dirs;   // D: folders, removed when empty
    QStringList keys;   // R: registry keys (Windows)

    void file(const QString &p) { files << QDir::cleanPath(p); }
    void dir(const QString &p) {
        const QString c = QDir::cleanPath(p);
        if (!dirs.contains(c)) dirs << c;
    }

    bool save(const QString &path, QString *error) const {
        QSaveFile f(path);
        if (!f.open(QIODevice::WriteOnly | QIODevice::Text)) {
            *error = QStringLiteral("Cannot write %1: %2").arg(path, f.errorString());
            return false;
        }
        QTextStream out(&f);
        out << "# OBCR " << version() << " installation manifest; used by the uninstaller.\n";
        for (const QString &p : dirs) out << "D " << p << '\n';
        for (const QString &p : files) out << "F " << p << '\n';
        for (const QString &k : keys) out << "R " << k << '\n';
        out.flush();
        if (!f.commit()) {
            *error = QStringLiteral("Cannot write %1: %2").arg(path, f.errorString());
            return false;
        }
        return true;
    }

    static Manifest load(const QString &path) {
        Manifest m;
        QFile f(path);
        if (!f.open(QIODevice::ReadOnly | QIODevice::Text)) return m;
        QTextStream in(&f);
        while (!in.atEnd()) {
            const QString line = in.readLine();
            if (line.size() < 3 || line[1] != ' ') continue;
            const QString value = line.mid(2);
            switch (line[0].toLatin1()) {
                case 'F': m.files << value; break;
                case 'D': m.dirs << value; break;
                case 'R': m.keys << value; break;
                default: break;
            }
        }
        return m;
    }
};

void report(const Progress &progress, int percent, const QString &message) {
    if (progress) progress(percent, message);
}

bool copyFile(const QString &from, const QString &to, QString *error) {
    QFile::remove(to);
    if (!QFile::copy(from, to)) {
        QFile src(from);
        *error = QStringLiteral("Cannot copy %1 to %2: %3").arg(from, to, src.errorString());
        return false;
    }
    // Resources and read-only sources give read-only copies.
    QFile::setPermissions(to, QFile::permissions(to) | QFileDevice::WriteOwner |
                                  QFileDevice::ReadOwner);
    return true;
}

void makeExecutable(const QString &path) {
    QFile::setPermissions(path, QFile::permissions(path) | QFileDevice::ExeOwner |
                                    QFileDevice::ExeGroup | QFileDevice::ExeOther |
                                    QFileDevice::ReadGroup | QFileDevice::ReadOther);
}

QString join(const QString &dir, const QString &name) {
    return QDir::cleanPath(dir + QLatin1Char('/') + name);
}

// Removes everything a manifest lists. Missing entries are fine; failures
// are collected.
void removeManifest(const Manifest &m, const Progress &progress, QStringList *failed) {
    const int total = int(m.files.size() + m.dirs.size() + m.keys.size());
    int done = 0;
    auto step = [&](const QString &what) {
        ++done;
        report(progress, total ? 100 * done / total : 100, what);
    };
    for (const QString &f : m.files) {
        const QFileInfo fi(f);
        if ((fi.exists() || fi.isSymLink()) && !QFile::remove(f)) *failed << f;
        step(QStringLiteral("Removed %1").arg(QDir::toNativeSeparators(f)));
    }
#ifdef Q_OS_WIN
    for (const QString &k : m.keys) {
        const int cut = int(k.lastIndexOf(QLatin1Char('\\')));
        QSettings parent(k.left(cut), QSettings::NativeFormat);
        parent.remove(k.mid(cut + 1));
        step(QStringLiteral("Removed registry entry %1").arg(k));
    }
#else
    done += int(m.keys.size());
#endif
    // Deepest folders first; only empty ones go.
    QStringList dirs = m.dirs;
    std::sort(dirs.begin(), dirs.end(),
              [](const QString &a, const QString &b) { return a.size() > b.size(); });
    for (const QString &d : dirs) {
        QDir().rmdir(d);
        step(QStringLiteral("Removed folder %1").arg(QDir::toNativeSeparators(d)));
    }
}

#ifdef Q_OS_LINUX
QString desktopEntry(const QString &name, const QString &comment, const QString &exec,
                     const QString &path, const QString &icon) {
    auto quote = [](QString s) {
        s.replace(QLatin1Char('\\'), QStringLiteral("\\\\"));
        s.replace(QLatin1Char('"'), QStringLiteral("\\\""));
        return QStringLiteral("\"%1\"").arg(s);
    };
    return QStringLiteral(
               "[Desktop Entry]\nType=Application\nName=%1\nGenericName=Operation Body Count\n"
               "Comment=%2\nExec=%3\nPath=%4\nIcon=%5\nTerminal=false\n"
               "Categories=Game;ActionGame;\n")
        .arg(name, comment, quote(exec), path, icon);
}

bool writeText(const QString &path, const QString &text, QString *error) {
    QSaveFile f(path);
    if (!f.open(QIODevice::WriteOnly | QIODevice::Text) || f.write(text.toUtf8()) < 0 ||
        !f.commit()) {
        *error = QStringLiteral("Cannot write %1: %2").arg(path, f.errorString());
        return false;
    }
    return true;
}
#endif

}  // namespace

QList<GameFile> DataCheck::toCopy() const {
    QList<GameFile> out;
    for (const GameFile &f : files)
        if ((f.role == QLatin1String("required") || f.role == QLatin1String("sequence")) &&
            f.status == QLatin1String("ok"))
            out << f;
    return out;
}

QStringList DataCheck::problems() const {
    QStringList out;
    for (const GameFile &f : files)
        if (f.role == QLatin1String("required") && f.status != QLatin1String("ok"))
            out << QStringLiteral("%1 (%2)").arg(
                f.name, f.status == QLatin1String("missing") ? QStringLiteral("missing")
                                                             : QStringLiteral("different version"));
    return out;
}

QString version() { return QStringLiteral(OBCR_VERSION); }

QString gameExeName() {
#ifdef Q_OS_WIN
    return QStringLiteral("obcr.exe");
#else
    return QStringLiteral("obcr");
#endif
}

QString uninstallerName() {
#ifdef Q_OS_WIN
    return QStringLiteral("OBCR-Uninstall.exe");
#else
    return QStringLiteral("OBCR-Uninstall");
#endif
}

QString defaultInstallDir() {
#ifdef Q_OS_WIN
    // %LOCALAPPDATA%\Programs\OBCR, where per-user programs go.
    const QString local = QStandardPaths::writableLocation(QStandardPaths::GenericDataLocation);
    return QDir::toNativeSeparators(join(local, QStringLiteral("Programs/OBCR")));
#else
    return join(QDir::homePath(), QStringLiteral(".local/opt/OBCR"));
#endif
}

QStringList dataSuggestions() {
    const QString game = QStringLiteral("steamapps/common/Operation Body Count/C/BCCD");
    QStringList roots;
#ifdef Q_OS_WIN
    roots << QStringLiteral("C:/Program Files (x86)/Steam") << QStringLiteral("C:/Program Files/Steam");
#else
    roots << join(QDir::homePath(), QStringLiteral(".steam/steam"))
          << join(QDir::homePath(), QStringLiteral(".local/share/Steam"));
#endif
    QStringList out;
    for (const QString &r : roots) {
        const QString d = join(r, game);
        if (QFileInfo::exists(join(d, QStringLiteral("GFXTILES.BC"))) ||
            QFileInfo::exists(join(d, QStringLiteral("gfxtiles.bc"))))
            out << QDir::toNativeSeparators(d);
    }
    return out;
}

QStringList userDataDirs() {
    // The `directories` crate's ProjectDirs::from("", "", "OBCR").
#ifdef Q_OS_WIN
    return {QDir::toNativeSeparators(join(qEnvironmentVariable("APPDATA"), QStringLiteral("OBCR")))};
#else
    return {join(QStandardPaths::writableLocation(QStandardPaths::GenericConfigLocation),
                 QStringLiteral("obcr")),
            join(QStandardPaths::writableLocation(QStandardPaths::GenericDataLocation),
                 QStringLiteral("obcr"))};
#endif
}

QString bundledGame(QString *error) {
    static std::unique_ptr<QTemporaryDir> dir;
    static QString path;
    if (!path.isEmpty()) return path;
    dir = std::make_unique<QTemporaryDir>();
    if (!dir->isValid()) {
        *error = QStringLiteral("Cannot create a temporary folder: %1").arg(dir->errorString());
        return {};
    }
    const QString out = dir->filePath(gameExeName());
    if (!copyFile(QStringLiteral(":/payload/") + gameExeName(), out, error)) return {};
    makeExecutable(out);
    path = out;
    return path;
}

DataCheck checkData(const QString &dir) {
    DataCheck c;
    c.dir = dir;
    if (dir.trimmed().isEmpty()) {
        c.error = QStringLiteral("Choose the folder of your Operation Body Count copy.");
        return c;
    }
    if (!QFileInfo(dir).isDir()) {
        c.error = QStringLiteral("This folder does not exist.");
        return c;
    }
    QString err;
    const QString game = bundledGame(&err);
    if (game.isEmpty()) {
        c.error = err;
        return c;
    }
    QProcess p;
    p.start(game, {QStringLiteral("--check-data"), dir});
    if (!p.waitForFinished(60000)) {
        c.error = QStringLiteral("Checking the files failed: %1").arg(p.errorString());
        p.kill();
        return c;
    }
    const QStringList lines =
        QString::fromUtf8(p.readAllStandardOutput()).split(QLatin1Char('\n'), Qt::SkipEmptyParts);
    for (QString line : lines) {
        line.remove(QLatin1Char('\r'));
        const QStringList t = line.split(QLatin1Char('\t'));
        if (t[0] == QLatin1String("error") && t.size() > 1) c.error = t[1];
        if (t[0] == QLatin1String("release") && t.size() > 2) c.release = t[2];
        if (t[0] == QLatin1String("file") && t.size() > 4) c.files << GameFile{t[1], t[2], t[3], t[4]};
        if (t[0] == QLatin1String("complete") && t.size() > 1) c.complete = t[1] == QLatin1String("yes");
    }
    if (c.files.isEmpty() && c.error.isEmpty())
        c.error = QStringLiteral("The game check gave no answer (exit code %1).").arg(p.exitCode());
    return c;
}

Existing findExisting() {
    QSettings s(kSettingsOrg, kSettingsApp);
    Existing e{s.value(QStringLiteral("installDir")).toString(),
               s.value(QStringLiteral("version")).toString()};
    if (!isInstallation(e.dir)) return {};
    return e;
}

bool isInstallation(const QString &dir) {
    return !dir.isEmpty() && QFileInfo::exists(join(dir, kManifest));
}

bool isUsableTarget(const QString &dir, QString *why) {
    const QFileInfo fi(dir);
    if (dir.trimmed().isEmpty() || !fi.isAbsolute()) {
        *why = QStringLiteral("Choose a full folder path.");
        return false;
    }
    if (fi.exists() && !fi.isDir()) {
        *why = QStringLiteral("This is a file, not a folder.");
        return false;
    }
    if (fi.exists() && !isInstallation(dir) &&
        !QDir(dir).entryList(QDir::AllEntries | QDir::NoDotAndDotDot | QDir::Hidden).isEmpty()) {
        *why = QStringLiteral("This folder is not empty. Choose an empty or new folder.");
        return false;
    }
    return true;
}

namespace {

// The runtime files the uninstaller copy needs (from the packaging list).
QStringList runtimeFiles() {
    QStringList runtime;
    QFile list(join(QCoreApplication::applicationDirPath(), kRuntimeList));
    if (list.open(QIODevice::ReadOnly | QIODevice::Text)) {
        QTextStream in(&list);
        while (!in.atEnd()) {
            const QString line = in.readLine().trimmed();
            if (!line.isEmpty() && !line.startsWith(QLatin1Char('#')) &&
                !line.contains(QStringLiteral("..")))
                runtime << line;
        }
    }
    return runtime;
}

}  // namespace

qint64 installSize(const DataCheck &check) {
    qint64 bytes = QFileInfo(QStringLiteral(":/payload/") + gameExeName()).size();
    bytes += QFileInfo(QCoreApplication::applicationFilePath()).size();
    for (const QString &rel : runtimeFiles())
        bytes += QFileInfo(join(QCoreApplication::applicationDirPath(), rel)).size();
    for (const GameFile &f : check.toCopy()) bytes += QFileInfo(f.path).size();
    return bytes;
}

bool install(const Options &o, const DataCheck &check, const Progress &progress, QString *error) {
    const QString target = QDir::cleanPath(o.targetDir);
    QString why;
    if (!isUsableTarget(target, &why)) {
        *error = why;
        return false;
    }
    if (!check.complete) {
        *error = QStringLiteral("The game files are incomplete.");
        return false;
    }
    // Updating: remove the previous installation first (settings and saved
    // games are elsewhere and stay).
    if (isInstallation(target)) {
        report(progress, 0, QStringLiteral("Removing the previous installation"));
        if (!uninstall(target, false, nullptr, error)) return false;
    }

    Manifest m;
    bool ok = false;
    auto fail = [&](const QString &message) {
        *error = message;
        QStringList ignored;
        removeManifest(m, nullptr, &ignored);
        return false;
    };

    const QString data = join(target, kDataDir);
    const QString uninstDir = join(target, kUninstallDir);
    if (!QDir().mkpath(data)) return fail(QStringLiteral("Cannot create %1").arg(data));
    m.dir(target);
    m.dir(data);

    // The game.
    report(progress, 5, QStringLiteral("Installing OBCR"));
    const QString exe = join(target, gameExeName());
    if (!copyFile(QStringLiteral(":/payload/") + gameExeName(), exe, error)) return fail(*error);
    makeExecutable(exe);
    m.file(exe);

    // The game files.
    const QList<GameFile> files = check.toCopy();
    for (int i = 0; i < files.size(); ++i) {
        const GameFile &f = files[i];
        report(progress, 10 + 50 * i / std::max<int>(1, int(files.size())),
               QStringLiteral("Copying %1").arg(f.name));
        const QString to = join(data, f.name);
        if (!copyFile(f.path, to, error)) return fail(*error);
        m.file(to);
    }

    // The uninstaller: this program and its runtime.
    report(progress, 65, QStringLiteral("Installing the uninstaller"));
    const QString selfDir = QCoreApplication::applicationDirPath();
    const QString selfExe = QCoreApplication::applicationFilePath();
    const QStringList runtime = runtimeFiles();
    if (!QDir().mkpath(uninstDir)) return fail(QStringLiteral("Cannot create %1").arg(uninstDir));
    m.dir(uninstDir);
    const QString uninstExe = join(uninstDir, uninstallerName());
    if (!copyFile(selfExe, uninstExe, error)) return fail(*error);
    makeExecutable(uninstExe);
    m.file(uninstExe);
    for (const QString &rel : runtime) {
        const QString from = join(selfDir, rel);
        if (QFileInfo(from).absoluteFilePath() == QFileInfo(selfExe).absoluteFilePath()) continue;
        const QString to = join(uninstDir, rel);
        QString parent = QFileInfo(to).absolutePath();
        while (parent.size() > uninstDir.size()) {
            m.dir(parent);
            parent = QFileInfo(parent).absolutePath();
        }
        QDir().mkpath(QFileInfo(to).absolutePath());
        if (!copyFile(from, to, error)) return fail(*error);
        if (QFileInfo(from).isExecutable()) makeExecutable(to);
        m.file(to);
    }

    // The icon (generated).
    const QString iconPath = join(target, kIconFile);
    if (!icon(256).save(iconPath, "PNG")) return fail(QStringLiteral("Cannot write %1").arg(iconPath));
    m.file(iconPath);

    report(progress, 80, QStringLiteral("Creating shortcuts"));
#ifdef Q_OS_WIN
    if (o.menuEntries) {
        const QString menu = join(QStandardPaths::writableLocation(QStandardPaths::ApplicationsLocation),
                                  QStringLiteral("OBCR"));
        if (!QDir().mkpath(menu)) return fail(QStringLiteral("Cannot create %1").arg(menu));
        m.dir(menu);
        const QString game = join(menu, QStringLiteral("OBCR.lnk"));
        const QString remove = join(menu, QStringLiteral("Uninstall OBCR.lnk"));
        QFile::remove(game);
        QFile::remove(remove);
        if (!QFile::link(exe, game)) return fail(QStringLiteral("Cannot create %1").arg(game));
        m.file(game);
        if (!QFile::link(uninstExe, remove)) return fail(QStringLiteral("Cannot create %1").arg(remove));
        m.file(remove);
    }
    if (o.desktopShortcut) {
        const QString link = join(QStandardPaths::writableLocation(QStandardPaths::DesktopLocation),
                                  QStringLiteral("OBCR.lnk"));
        QFile::remove(link);
        if (!QFile::link(exe, link)) return fail(QStringLiteral("Cannot create %1").arg(link));
        m.file(link);
    }
    {
        QSettings reg(kUninstallKey, QSettings::NativeFormat);
        reg.setValue(QStringLiteral("DisplayName"), QStringLiteral("OBCR (Operation Body Count)"));
        reg.setValue(QStringLiteral("DisplayVersion"), version());
        reg.setValue(QStringLiteral("Publisher"), QStringLiteral("OBCR project"));
        reg.setValue(QStringLiteral("InstallLocation"), QDir::toNativeSeparators(target));
        reg.setValue(QStringLiteral("DisplayIcon"), QDir::toNativeSeparators(exe));
        const QString un = QStringLiteral("\"%1\"").arg(QDir::toNativeSeparators(uninstExe));
        reg.setValue(QStringLiteral("UninstallString"), un);
        reg.setValue(QStringLiteral("QuietUninstallString"), un + QStringLiteral(" --unattended"));
        reg.setValue(QStringLiteral("NoModify"), 1);
        reg.setValue(QStringLiteral("NoRepair"), 1);
        qint64 bytes = 0;
        for (const QString &f : m.files) bytes += QFileInfo(f).size();
        reg.setValue(QStringLiteral("EstimatedSize"), int(bytes / 1024));
        reg.sync();
        if (reg.status() != QSettings::NoError)
            return fail(QStringLiteral("Cannot register the uninstaller with Windows."));
        m.keys << QString::fromLatin1(kUninstallKey);
    }
#else
    if (o.menuEntries) {
        const QString apps = QStandardPaths::writableLocation(QStandardPaths::ApplicationsLocation);
        QDir().mkpath(apps);
        const QString game = join(apps, QStringLiteral("obcr.desktop"));
        const QString remove = join(apps, QStringLiteral("obcr-uninstall.desktop"));
        if (!writeText(game, desktopEntry(QStringLiteral("OBCR"),
                                          QStringLiteral("A modern source port of Operation Body Count"),
                                          exe, target, iconPath),
                       error))
            return fail(*error);
        m.file(game);
        if (!writeText(remove, desktopEntry(QStringLiteral("Uninstall OBCR"),
                                            QStringLiteral("Remove OBCR from this computer"),
                                            uninstExe, uninstDir, iconPath),
                       error))
            return fail(*error);
        m.file(remove);
        QProcess::execute(QStringLiteral("update-desktop-database"), {apps});
    }
    if (o.desktopShortcut) {
        const QString desk = QStandardPaths::writableLocation(QStandardPaths::DesktopLocation);
        const QString link = join(desk, QStringLiteral("obcr.desktop"));
        if (QDir(desk).exists()) {
            if (!writeText(link, desktopEntry(QStringLiteral("OBCR"),
                                              QStringLiteral("A modern source port of Operation Body Count"),
                                              exe, target, iconPath),
                           error))
                return fail(*error);
            makeExecutable(link);
            m.file(link);
        }
    }
    if (o.pathLink) {
        const QString bin = join(QDir::homePath(), QStringLiteral(".local/bin"));
        const QString link = join(bin, QStringLiteral("obcr"));
        const QFileInfo li(link);
        // Never replace a file that is not a link (someone else's obcr).
        if (!li.exists() || li.isSymLink()) {
            QDir().mkpath(bin);
            QFile::remove(link);
            if (QFile::link(exe, link)) m.file(link);
        }
    }
#endif

    report(progress, 95, QStringLiteral("Writing the installation record"));
    QSettings s(kSettingsOrg, kSettingsApp);
    s.setValue(QStringLiteral("installDir"), target);
    s.setValue(QStringLiteral("version"), version());
    s.sync();
    if (!m.save(join(target, kManifest), error)) return fail(*error);
    report(progress, 100, QStringLiteral("OBCR %1 is installed in %2").arg(version(), QDir::toNativeSeparators(target)));
    ok = true;
    return ok;
}

bool uninstall(const QString &dir, bool purgeUserData, const Progress &progress, QString *error) {
    const QString target = QDir::cleanPath(dir);
    const QString manifestPath = join(target, kManifest);
    if (!QFileInfo::exists(manifestPath)) {
        *error = QStringLiteral("No OBCR installation found in %1").arg(QDir::toNativeSeparators(target));
        return false;
    }
    const Manifest m = Manifest::load(manifestPath);
    QStringList failed;
    removeManifest(m, progress, &failed);
    QFile::remove(manifestPath);
    QDir().rmdir(target);
    if (purgeUserData)
        for (const QString &d : userDataDirs()) {
            report(progress, 100, QStringLiteral("Removing %1").arg(d));
            if (QFileInfo::exists(d) && !QDir(d).removeRecursively()) failed << d;
        }
    QSettings s(kSettingsOrg, kSettingsApp);
    if (QDir::cleanPath(s.value(QStringLiteral("installDir")).toString()) == target) s.clear();
    // The running uninstaller cannot remove itself on Windows; main() runs
    // it from a temporary copy, so anything left here is a real failure.
    if (!failed.isEmpty()) {
        *error = QStringLiteral("These could not be removed:\n%1").arg(failed.join(QLatin1Char('\n')));
        return false;
    }
    report(progress, 100, QStringLiteral("OBCR has been removed."));
    return true;
}

QImage icon(int size) {
    QImage img(size, size, QImage::Format_ARGB32_Premultiplied);
    img.fill(Qt::transparent);
    QPainter p(&img);
    p.setRenderHint(QPainter::Antialiasing);
    const qreal s = size;
    QPainterPath body;
    body.addRoundedRect(QRectF(s * 0.04, s * 0.04, s * 0.92, s * 0.92), s * 0.18, s * 0.18);
    QLinearGradient bg(0, 0, 0, s);
    bg.setColorAt(0, QColor(0x2b, 0x31, 0x3a));
    bg.setColorAt(1, QColor(0x10, 0x12, 0x16));
    p.fillPath(body, bg);
    // A crosshair.
    QPen pen(QColor(0xe8, 0xe8, 0xe8), s * 0.045, Qt::SolidLine, Qt::RoundCap);
    p.setPen(pen);
    const QPointF c(s * 0.5, s * 0.44);
    const qreal r = s * 0.25;
    p.drawEllipse(c, r, r);
    p.drawLine(QPointF(c.x(), c.y() - r * 1.35), QPointF(c.x(), c.y() - r * 0.45));
    p.drawLine(QPointF(c.x(), c.y() + r * 0.45), QPointF(c.x(), c.y() + r * 1.35));
    p.drawLine(QPointF(c.x() - r * 1.35, c.y()), QPointF(c.x() - r * 0.45, c.y()));
    p.drawLine(QPointF(c.x() + r * 0.45, c.y()), QPointF(c.x() + r * 1.35, c.y()));
    p.setPen(Qt::NoPen);
    p.setBrush(QColor(0xd0, 0x22, 0x22));
    p.drawEllipse(c, s * 0.05, s * 0.05);
    QFont f;
    f.setBold(true);
    f.setPixelSize(int(s * 0.17));
    p.setFont(f);
    p.setPen(QColor(0xe8, 0xe8, 0xe8));
    p.drawText(QRectF(0, s * 0.74, s, s * 0.2), Qt::AlignCenter, QStringLiteral("OBCR"));
    return img;
}

}  // namespace setup
