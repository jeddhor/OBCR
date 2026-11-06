// OBCR Setup: installation logic shared by the wizard and unattended mode.
// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once

#include <QImage>
#include <QList>
#include <QString>
#include <QStringList>

#include <functional>

namespace setup {

// One file of the user's game copy, as `obcr --check-data` reports it.
struct GameFile {
    QString role;    // required, sequence, user, extra
    QString status;  // ok, missing, mismatch, present
    QString name;    // canonical upper-case name
    QString path;    // where it was found
};

// Result of checking a directory of game files with the bundled `obcr`.
struct DataCheck {
    QString dir;
    QString error;     // set when the check could not run
    QString release;   // release description
    bool complete = false;
    QList<GameFile> files;

    // Files OBCR uses that are present and verified: required files and
    // rendered sequences.
    QList<GameFile> toCopy() const;
    // Required files that are missing or do not match the known release.
    QStringList problems() const;
};

// Installer choices.
struct Options {
    QString dataDir;
    QString targetDir;
    bool menuEntries = true;     // Start Menu / application menu
    bool desktopShortcut = false;
    bool pathLink = true;        // ~/.local/bin/obcr (Linux)
};

// A previous installation recorded by Setup.
struct Existing {
    QString dir;
    QString version;
    bool valid() const { return !dir.isEmpty(); }
};

using Progress = std::function<void(int percent, const QString &message)>;

QString version();
QString gameExeName();      // obcr.exe or obcr
QString uninstallerName();  // OBCR-Uninstall(.exe)
QString defaultInstallDir();
// Directories that look like game copies (Steam default locations).
QStringList dataSuggestions();
// Settings and saved games written by OBCR itself (never by Setup).
QStringList userDataDirs();

// Extracts the bundled game binary to a private temporary directory.
QString bundledGame(QString *error);
DataCheck checkData(const QString &dir);

Existing findExisting();
// True if `dir` holds an installation made by Setup.
bool isInstallation(const QString &dir);
// True if `dir` can be installed into: missing, empty or an installation.
bool isUsableTarget(const QString &dir, QString *why);

// Bytes an installation takes: the game, the game files, the uninstaller.
qint64 installSize(const DataCheck &check);

bool install(const Options &options, const DataCheck &check, const Progress &progress,
             QString *error);
bool uninstall(const QString &dir, bool purgeUserData, const Progress &progress, QString *error);

// Renders the OBCR icon (generated, not taken from the game).
QImage icon(int size);

}  // namespace setup
