// OBCR Setup: a Qt 6 installer and uninstaller for OBCR, per user, on
// Windows and Linux.
//
//   OBCR-Setup                         installation wizard
//   OBCR-Setup --unattended --data DIR [--target DIR] [--desktop-shortcut]
//              [--no-menu] [--no-path-link]
//   OBCR-Setup --uninstall [--target DIR] [--unattended] [--purge]
//   OBCR-Uninstall ...                 same as --uninstall (installed copy)
//
// SPDX-License-Identifier: GPL-3.0-or-later
#include <QApplication>
#include <QCommandLineParser>
#include <QDir>
#include <QDirIterator>
#include <QFile>
#include <QFileInfo>
#include <QIcon>
#include <QPixmap>
#include <QProcess>
#include <QTextStream>
#include <QThread>
#include <QWizard>

#include <cstdio>

#include "setup.h"
#include "wizard.h"

#ifdef Q_OS_WIN
#include <windows.h>
#endif

namespace {

QTextStream &out() {
    static QTextStream s(stdout);
    return s;
}

setup::Progress printer() {
    return [](int percent, const QString &message) {
        out() << QStringLiteral("[%1%] ").arg(percent, 3) << message << Qt::endl;
    };
}

bool hasArg(int argc, char **argv, const char *name) {
    for (int i = 1; i < argc; ++i)
        if (qstrcmp(argv[i], name) == 0) return true;
    return false;
}

#ifdef Q_OS_WIN
// Windows cannot delete a running program: copy the uninstaller to a
// temporary folder and run it from there. Returns false if that failed.
bool relaunchFromTemp(const QString &installDir, const QStringList &args) {
    const QString from = QCoreApplication::applicationDirPath();
    const QString to = QDir::temp().filePath(
        QStringLiteral("obcr-uninstall-%1").arg(QCoreApplication::applicationPid()));
    QDir(to).removeRecursively();
    QDirIterator it(from, QDir::Files, QDirIterator::Subdirectories);
    while (it.hasNext()) {
        const QString f = it.next();
        const QString dest = to + QLatin1Char('/') + QDir(from).relativeFilePath(f);
        QDir().mkpath(QFileInfo(dest).absolutePath());
        if (!QFile::copy(f, dest)) return false;
    }
    const QString exe = to + QLatin1Char('/') + QFileInfo(QCoreApplication::applicationFilePath()).fileName();
    QStringList a = args;
    a << QStringLiteral("--target") << installDir << QStringLiteral("--cleanup-self");
    return QProcess::startDetached(exe, a, QDir::tempPath());
}

// Removes this temporary uninstaller copy shortly after it exits.
void scheduleSelfDelete() {
    const QString dir = QDir::toNativeSeparators(QCoreApplication::applicationDirPath());
    QProcess p;
    p.setProgram(QStringLiteral("cmd.exe"));
    // Not the folder being removed: a process's working folder cannot go.
    p.setWorkingDirectory(QDir::tempPath());
    p.setNativeArguments(
        QStringLiteral("/c ping -n 3 127.0.0.1 >nul & rd /s /q \"%1\"").arg(dir));
    p.setCreateProcessArgumentsModifier(
        [](QProcess::CreateProcessArguments *a) { a->flags |= CREATE_NO_WINDOW; });
    p.startDetached();
}
#endif

// Development aid: renders the wizard pages to PNG files.
int screenshots(const QString &outDir, const QString &dataDir) {
    QDir().mkpath(outDir);
    SetupWizard w;
    w.show();
    int n = 0;
    auto shot = [&](const char *name) {
        QApplication::processEvents();
        w.grab().save(QDir(outDir).filePath(QStringLiteral("%1-%2.png").arg(++n).arg(QLatin1String(name))));
    };
    shot("welcome");
    w.next();
    if (!dataDir.isEmpty()) w.dataPage()->setDir(dataDir);
    QApplication::processEvents();
    // Let the debounced check run.
    for (int i = 0; i < 20; ++i) {
        QThread::msleep(50);
        QApplication::processEvents();
    }
    shot("game-files");
    if (w.currentPage()->isComplete()) {
        w.next();
        shot("folder");
    }
    w.hide();
    UninstallDialog u(setup::defaultInstallDir());
    u.show();
    QApplication::processEvents();
    u.grab().save(QDir(outDir).filePath(QStringLiteral("%1-uninstall.png").arg(++n)));
    out() << "wrote " << n << " screenshots to " << outDir << Qt::endl;
    return 0;
}

}  // namespace

int main(int argc, char **argv) {
#ifdef Q_OS_LINUX
    // Unattended runs work without a display.
    const bool headless = hasArg(argc, argv, "--unattended") || hasArg(argc, argv, "--screenshots");
    if (headless && qEnvironmentVariableIsEmpty("DISPLAY") && qEnvironmentVariableIsEmpty("WAYLAND_DISPLAY"))
        qputenv("QT_QPA_PLATFORM", "offscreen");
#endif
    QApplication app(argc, argv);
    QApplication::setApplicationName(QStringLiteral("OBCR Setup"));
    QApplication::setOrganizationName(QStringLiteral("OBCR"));
    QApplication::setApplicationVersion(setup::version());
    QApplication::setWindowIcon(QIcon(QPixmap::fromImage(setup::icon(128))));

    QCommandLineParser parser;
    parser.setApplicationDescription(QStringLiteral("Installs or removes OBCR for this user."));
    parser.addHelpOption();
    parser.addVersionOption();
    const QCommandLineOption uninstallOpt(QStringLiteral("uninstall"), QStringLiteral("Remove OBCR."));
    const QCommandLineOption unattendedOpt(QStringLiteral("unattended"),
                                           QStringLiteral("No windows; report progress on standard output."));
    const QCommandLineOption dataOpt(QStringLiteral("data"), QStringLiteral("Your Operation Body Count folder."),
                                     QStringLiteral("dir"));
    const QCommandLineOption targetOpt(QStringLiteral("target"), QStringLiteral("Installation folder."),
                                       QStringLiteral("dir"));
    const QCommandLineOption desktopOpt(QStringLiteral("desktop-shortcut"), QStringLiteral("Add a desktop shortcut."));
    const QCommandLineOption noMenuOpt(QStringLiteral("no-menu"), QStringLiteral("No Start/applications menu entries."));
    const QCommandLineOption noLinkOpt(QStringLiteral("no-path-link"), QStringLiteral("No ~/.local/bin/obcr link (Linux)."));
    const QCommandLineOption purgeOpt(QStringLiteral("purge"),
                                      QStringLiteral("With --uninstall: also delete settings and saved games."));
    const QCommandLineOption shotsOpt(QStringLiteral("screenshots"), QStringLiteral("Development: render the pages."),
                                      QStringLiteral("dir"));
    QCommandLineOption cleanupOpt(QStringLiteral("cleanup-self"));
    cleanupOpt.setFlags(QCommandLineOption::HiddenFromHelp);
    const std::initializer_list<const QCommandLineOption *> all{
        &uninstallOpt, &unattendedOpt, &dataOpt, &targetOpt, &desktopOpt,
        &noMenuOpt,    &noLinkOpt,     &purgeOpt, &shotsOpt, &cleanupOpt};
    for (const QCommandLineOption *o : all) parser.addOption(*o);
    parser.process(app);

    const bool unattended = parser.isSet(unattendedOpt);
    const QString self = QCoreApplication::applicationFilePath();
    const bool uninstallMode =
        parser.isSet(uninstallOpt) || QFileInfo(self).baseName().contains(QStringLiteral("Uninstall"), Qt::CaseInsensitive);

    if (uninstallMode) {
        QString dir = parser.value(targetOpt);
        if (dir.isEmpty()) {
            // The installed uninstaller lives in <install>/uninstall.
            const QString parent = QFileInfo(QCoreApplication::applicationDirPath()).absolutePath();
            dir = setup::isInstallation(parent) ? parent : setup::findExisting().dir;
        }
        dir = QDir::cleanPath(QDir::fromNativeSeparators(dir));
#ifdef Q_OS_WIN
        const QString here = QDir::cleanPath(QCoreApplication::applicationDirPath());
        if (!parser.isSet(cleanupOpt) && !dir.isEmpty() && here.startsWith(dir + QLatin1Char('/'), Qt::CaseInsensitive)) {
            QStringList args{QStringLiteral("--uninstall")};
            if (unattended) args << QStringLiteral("--unattended");
            if (parser.isSet(purgeOpt)) args << QStringLiteral("--purge");
            if (relaunchFromTemp(dir, args)) return 0;
        }
#endif
        int code = 0;
        if (unattended) {
            QString error;
            if (dir.isEmpty() || !setup::uninstall(dir, parser.isSet(purgeOpt), printer(), &error)) {
                out() << "error: " << (dir.isEmpty() ? QStringLiteral("no OBCR installation found") : error) << Qt::endl;
                code = 1;
            }
        } else {
            UninstallDialog d(dir);
            code = d.exec() == QDialog::Accepted ? 0 : 1;
        }
#ifdef Q_OS_WIN
        if (parser.isSet(cleanupOpt)) scheduleSelfDelete();
#endif
        return code;
    }

    if (parser.isSet(shotsOpt)) return screenshots(parser.value(shotsOpt), parser.value(dataOpt));

    if (unattended) {
        setup::Options o;
        o.dataDir = QDir::fromNativeSeparators(parser.value(dataOpt));
        o.targetDir = QDir::fromNativeSeparators(parser.isSet(targetOpt) ? parser.value(targetOpt)
                                                                         : setup::defaultInstallDir());
        o.menuEntries = !parser.isSet(noMenuOpt);
        o.desktopShortcut = parser.isSet(desktopOpt);
        o.pathLink = !parser.isSet(noLinkOpt);
        const setup::DataCheck check = setup::checkData(o.dataDir);
        if (!check.error.isEmpty() || !check.complete) {
            out() << "error: " << (check.error.isEmpty() ? QStringLiteral("incomplete game files: ") + check.problems().join(QStringLiteral(", "))
                                                         : check.error)
                  << Qt::endl;
            return 2;
        }
        out() << "game files: " << check.release << ", " << check.toCopy().size() << " to copy" << Qt::endl;
        QString error;
        if (!setup::install(o, check, printer(), &error)) {
            out() << "error: " << error << Qt::endl;
            return 1;
        }
        return 0;
    }

    SetupWizard w;
    w.show();
    return QApplication::exec();
}
