// OBCR Setup: the installation wizard and the uninstall dialog.
// SPDX-License-Identifier: GPL-3.0-or-later
#include "wizard.h"

#include <QCheckBox>
#include <QDialogButtonBox>
#include <QDir>
#include <QFile>
#include <QFileDialog>
#include <QFileInfo>
#include <QHBoxLayout>
#include <QLabel>
#include <QLineEdit>
#include <QLocale>
#include <QMessageBox>
#include <QPlainTextEdit>
#include <QProcess>
#include <QProgressBar>
#include <QPushButton>
#include <QRadioButton>
#include <QStorageInfo>
#include <QTabWidget>
#include <QTextBrowser>
#include <QThread>
#include <QTimer>
#include <QVBoxLayout>

namespace {

QLabel *paragraph(const QString &text, QWidget *parent) {
    auto *l = new QLabel(text, parent);
    l->setWordWrap(true);
    l->setTextFormat(Qt::RichText);
    l->setOpenExternalLinks(true);
    return l;
}

QString mb(qint64 bytes) {
    return QLocale().formattedDataSize(bytes, 1, QLocale::DataSizeTraditionalFormat);
}

void showLicense(QWidget *parent) {
    QDialog d(parent);
    d.setWindowTitle(QStringLiteral("OBCR licence"));
    d.resize(720, 560);
    auto *layout = new QVBoxLayout(&d);
    auto *tabs = new QTabWidget(&d);
    auto page = [&](const QString &resource, const QString &title, bool markdown) {
        auto *text = new QTextBrowser(tabs);
        text->setOpenExternalLinks(true);
        QFile f(resource);
        if (f.open(QIODevice::ReadOnly)) {
            const QString s = QString::fromUtf8(f.readAll());
            if (markdown) text->setMarkdown(s);
            else text->setPlainText(s);
        }
        tabs->addTab(text, title);
    };
    page(QStringLiteral(":/payload/LICENSE"), QStringLiteral("Licence (GPL-3.0)"), false);
    page(QStringLiteral(":/payload/THIRD_PARTY_NOTICES.md"), QStringLiteral("Third-party notices"), true);
    layout->addWidget(tabs);
    auto *buttons = new QDialogButtonBox(QDialogButtonBox::Close, &d);
    QObject::connect(buttons, &QDialogButtonBox::rejected, &d, &QDialog::reject);
    layout->addWidget(buttons);
    d.exec();
}

}  // namespace

// ---------------------------------------------------------------- Intro

IntroPage::IntroPage(SetupWizard *w) : QWizardPage(w), wizard_(w) {
    setTitle(QStringLiteral("Welcome to OBCR Setup"));
    setSubTitle(QStringLiteral("OBCR %1, a modern source port of Operation Body Count (1994).")
                    .arg(setup::version()));
    auto *layout = new QVBoxLayout(this);
    layout->addWidget(paragraph(
        QStringLiteral(
            "<p>This installs OBCR for your user account only; no administrator rights are "
            "needed.</p>"
            "<p>OBCR needs the data files of <b>your own copy</b> of Operation Body Count (the "
            "floppy release, files dated 1996-12-24). Setup checks them and copies the ones the "
            "game uses into the installation folder; your original copy is left untouched.</p>"
            "<p>OBCR is free software under the GNU General Public License, version 3 or later. "
            "It comes with no warranty.</p>"),
        this));
    auto *license = new QPushButton(QStringLiteral("View licence..."), this);
    connect(license, &QPushButton::clicked, this, [this] { showLicense(this); });
    auto *row = new QHBoxLayout;
    row->addWidget(license);
    row->addStretch();
    layout->addLayout(row);
    if (w->existing.valid()) {
        layout->addSpacing(12);
        layout->addWidget(paragraph(
            QStringLiteral("OBCR %1 is already installed in <b>%2</b>.")
                .arg(w->existing.version.toHtmlEscaped(),
                     QDir::toNativeSeparators(w->existing.dir).toHtmlEscaped()),
            this));
        update_ = new QRadioButton(
            QStringLiteral("Update or repair it (settings and saved games are kept)"), this);
        remove_ = new QRadioButton(QStringLiteral("Remove OBCR from this computer"), this);
        update_->setChecked(true);
        layout->addWidget(update_);
        layout->addWidget(remove_);
    }
    layout->addStretch();
}

bool IntroPage::validatePage() {
    if (remove_ && remove_->isChecked()) {
        wizard_->hide();
        UninstallDialog d(wizard_->existing.dir);
        d.exec();
        wizard_->reject();
        return false;
    }
    return true;
}

// ---------------------------------------------------------------- Game files

DataPage::DataPage(SetupWizard *w) : QWizardPage(w), wizard_(w) {
    setTitle(QStringLiteral("Your game files"));
    setSubTitle(QStringLiteral(
        "Choose the folder of your Operation Body Count copy: the one with COUNT.EXE and the "
        ".BC files."));
    auto *layout = new QVBoxLayout(this);
    auto *row = new QHBoxLayout;
    dir_ = new QLineEdit(this);
    dir_->setPlaceholderText(QStringLiteral("Folder with COUNT.EXE, GFXTILES.BC, ..."));
    auto *browse = new QPushButton(QStringLiteral("Browse..."), this);
    row->addWidget(dir_);
    row->addWidget(browse);
    layout->addLayout(row);
    status_ = paragraph(QString(), this);
    status_->setTextInteractionFlags(Qt::TextSelectableByMouse);
    layout->addWidget(status_);
    layout->addStretch();
    debounce_ = new QTimer(this);
    debounce_->setSingleShot(true);
    debounce_->setInterval(400);
    connect(debounce_, &QTimer::timeout, this, &DataPage::check);
    connect(dir_, &QLineEdit::textChanged, this, [this] {
        wizard_->check = {};
        status_->setText(QStringLiteral("<i>Checking...</i>"));
        emit completeChanged();
        debounce_->start();
    });
    connect(browse, &QPushButton::clicked, this, [this] {
        const QString d = QFileDialog::getExistingDirectory(
            this, QStringLiteral("Operation Body Count folder"), dir_->text());
        if (!d.isEmpty()) setDir(d);
    });
}

void DataPage::initializePage() {
    if (dir_->text().isEmpty()) {
        const QStringList found = setup::dataSuggestions();
        if (!found.isEmpty()) setDir(found.first());
        else check();
    }
}

void DataPage::setDir(const QString &dir) { dir_->setText(QDir::toNativeSeparators(dir)); }

void DataPage::check() {
    const QString dir = QDir::fromNativeSeparators(dir_->text().trimmed());
    setup::DataCheck c = setup::checkData(dir);
    wizard_->check = c;
    wizard_->options.dataDir = dir;
    if (!c.error.isEmpty()) {
        status_->setText(c.error.toHtmlEscaped());
    } else if (c.complete) {
        qint64 bytes = 0;
        const auto files = c.toCopy();
        int sequences = 0;
        for (const auto &f : files) {
            bytes += QFileInfo(f.path).size();
            if (f.role == QLatin1String("sequence")) ++sequences;
        }
        QString text = QStringLiteral("<p><b>Found:</b> %1.</p><p>%2 files (%3) will be copied")
                           .arg(c.release.toHtmlEscaped())
                           .arg(files.size())
                           .arg(mb(bytes));
        text += sequences ? QStringLiteral(", including %1 animated sequences.</p>").arg(sequences)
                          : QStringLiteral(". No animated sequences (F2.BC ...) were found; the "
                                           "game skips them, as the original does.</p>");
        status_->setText(text);
    } else {
        const QStringList problems = c.problems();
        status_->setText(
            QStringLiteral("<p><b>This folder is not a complete copy of the supported release.</b>"
                           "</p><p>%1</p>")
                .arg(problems.isEmpty() ? QStringLiteral("No game files were found here.")
                                        : QStringLiteral("Missing or different: ") +
                                              problems.join(QStringLiteral(", ")).toHtmlEscaped()));
    }
    emit completeChanged();
}

bool DataPage::isComplete() const { return wizard_->check.complete; }

// ---------------------------------------------------------------- Target

TargetPage::TargetPage(SetupWizard *w) : QWizardPage(w), wizard_(w) {
    setTitle(QStringLiteral("Installation folder"));
    setSubTitle(QStringLiteral("OBCR and its copy of your game files go into this folder."));
    setCommitPage(true);
    setButtonText(QWizard::CommitButton, QStringLiteral("&Install"));
    auto *layout = new QVBoxLayout(this);
    auto *row = new QHBoxLayout;
    dir_ = new QLineEdit(this);
    auto *browse = new QPushButton(QStringLiteral("Browse..."), this);
    row->addWidget(dir_);
    row->addWidget(browse);
    layout->addLayout(row);
    status_ = paragraph(QString(), this);
    layout->addWidget(status_);
    layout->addSpacing(8);
#ifdef Q_OS_WIN
    menu_ = new QCheckBox(QStringLiteral("Add OBCR to the Start menu"), this);
    desktop_ = new QCheckBox(QStringLiteral("Put a shortcut on the desktop"), this);
#else
    menu_ = new QCheckBox(QStringLiteral("Add OBCR to the applications menu"), this);
    desktop_ = new QCheckBox(QStringLiteral("Put a launcher on the desktop"), this);
    pathLink_ = new QCheckBox(QStringLiteral("Make the obcr command available (~/.local/bin/obcr)"), this);
#endif
    menu_->setChecked(true);
    layout->addWidget(menu_);
    layout->addWidget(desktop_);
    if (pathLink_) {
        pathLink_->setChecked(true);
        layout->addWidget(pathLink_);
    }
    layout->addStretch();
    connect(dir_, &QLineEdit::textChanged, this, &TargetPage::update);
    connect(browse, &QPushButton::clicked, this, [this] {
        const QString d = QFileDialog::getExistingDirectory(
            this, QStringLiteral("Installation folder"), dir_->text());
        if (!d.isEmpty()) dir_->setText(QDir::toNativeSeparators(d));
    });
}

void TargetPage::initializePage() {
    if (dir_->text().isEmpty())
        dir_->setText(QDir::toNativeSeparators(wizard_->existing.valid() ? wizard_->existing.dir
                                                                          : setup::defaultInstallDir()));
    update();
}

void TargetPage::update() {
    const QString dir = QDir::fromNativeSeparators(dir_->text().trimmed());
    QString why;
    if (!setup::isUsableTarget(dir, &why)) {
        status_->setText(QStringLiteral("<span style='color:#b00020'>%1</span>").arg(why.toHtmlEscaped()));
    } else {
        const qint64 need = setup::installSize(wizard_->check);
        QString existing = dir;
        while (!existing.isEmpty() && !QFileInfo::exists(existing)) existing = QFileInfo(existing).path();
        const QStorageInfo storage(existing);
        QString text = QStringLiteral("About %1 needed, %2 free.").arg(mb(need), mb(storage.bytesAvailable()));
        if (setup::isInstallation(dir))
            text += QStringLiteral(" The OBCR installation in this folder will be replaced.");
        status_->setText(text);
    }
    emit completeChanged();
}

bool TargetPage::isComplete() const {
    QString why;
    return setup::isUsableTarget(QDir::fromNativeSeparators(dir_->text().trimmed()), &why);
}

bool TargetPage::validatePage() {
    wizard_->options.targetDir = QDir::fromNativeSeparators(dir_->text().trimmed());
    wizard_->options.menuEntries = menu_->isChecked();
    wizard_->options.desktopShortcut = desktop_->isChecked();
    wizard_->options.pathLink = pathLink_ && pathLink_->isChecked();
    return true;
}

// ---------------------------------------------------------------- Install

InstallPage::InstallPage(SetupWizard *w) : QWizardPage(w), wizard_(w) {
    setTitle(QStringLiteral("Installing"));
    setSubTitle(QStringLiteral("Copying OBCR and your game files."));
    auto *layout = new QVBoxLayout(this);
    bar_ = new QProgressBar(this);
    bar_->setRange(0, 100);
    status_ = paragraph(QString(), this);
    log_ = new QPlainTextEdit(this);
    log_->setReadOnly(true);
    layout->addWidget(bar_);
    layout->addWidget(status_);
    layout->addWidget(log_);
}

void InstallPage::initializePage() {
    const setup::Options options = wizard_->options;
    const setup::DataCheck check = wizard_->check;
    auto progress = [this](int percent, const QString &message) {
        QMetaObject::invokeMethod(
            this,
            [this, percent, message] {
                bar_->setValue(percent);
                status_->setText(message.toHtmlEscaped());
                log_->appendPlainText(message);
            },
            Qt::QueuedConnection);
    };
    QThread *thread = QThread::create([this, options, check, progress] {
        QString error;
        const bool ok = setup::install(options, check, progress, &error);
        QMetaObject::invokeMethod(
            this,
            [this, ok, error] {
                done_ = true;
                ok_ = ok;
                if (!ok) {
                    status_->setText(
                        QStringLiteral("<span style='color:#b00020'><b>Installation failed.</b> %1"
                                       "</span><br>Nothing was left behind; you can run Setup again.")
                            .arg(error.toHtmlEscaped()));
                    log_->appendPlainText(QStringLiteral("Failed: ") + error);
                } else {
                    wizard_->next();
                }
                emit completeChanged();
            },
            Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
}

bool InstallPage::isComplete() const { return done_ && ok_; }

// ---------------------------------------------------------------- Finish

FinishPage::FinishPage(SetupWizard *w) : QWizardPage(w), wizard_(w) {
    setTitle(QStringLiteral("OBCR is installed"));
    auto *layout = new QVBoxLayout(this);
    text_ = paragraph(QString(), this);
    start_ = new QCheckBox(QStringLiteral("Start OBCR now"), this);
    start_->setChecked(true);
    layout->addWidget(text_);
    layout->addSpacing(8);
    layout->addWidget(start_);
    layout->addStretch();
}

void FinishPage::initializePage() {
    const QString dir = QDir::toNativeSeparators(wizard_->options.targetDir);
#ifdef Q_OS_WIN
    const QString remove = QStringLiteral(
        "To remove it later, use <i>Settings &gt; Apps</i>, the <i>Uninstall OBCR</i> entry in the "
        "Start menu, or run Setup again.");
#else
    const QString remove = QStringLiteral(
        "To remove it later, use <i>Uninstall OBCR</i> in the applications menu, run "
        "<i>uninstall/OBCR-Uninstall</i> in the installation folder, or run Setup again.");
#endif
    text_->setText(QStringLiteral("<p>OBCR %1 is installed in <b>%2</b>.</p><p>Options (resolution, "
                                  "textures, controls) are in the game: press <b>O</b> in the main "
                                  "menu.</p><p>%3</p>")
                       .arg(setup::version(), dir.toHtmlEscaped(), remove));
}

bool FinishPage::startGame() const { return start_->isChecked(); }

// ---------------------------------------------------------------- Wizard

SetupWizard::SetupWizard(QWidget *parent) : QWizard(parent) {
    existing = setup::findExisting();
    setWindowTitle(QStringLiteral("OBCR Setup"));
    setWizardStyle(QWizard::ModernStyle);
    setPixmap(QWizard::LogoPixmap, QPixmap::fromImage(setup::icon(64)));
    setOption(QWizard::NoBackButtonOnLastPage, true);
    setOption(QWizard::NoCancelButtonOnLastPage, true);
    resize(680, 500);
    setPage(Intro, new IntroPage(this));
    data_ = new DataPage(this);
    setPage(Data, data_);
    setPage(Target, new TargetPage(this));
    setPage(Install, new InstallPage(this));
    finish_ = new FinishPage(this);
    setPage(Finish, finish_);
    setStartId(Intro);
}

void SetupWizard::accept() {
    if (currentId() == Finish && finish_->startGame()) {
        const QString dir = options.targetDir;
        QProcess::startDetached(QDir(dir).filePath(setup::gameExeName()), {}, dir);
    }
    QWizard::accept();
}

// ---------------------------------------------------------------- Uninstall

UninstallDialog::UninstallDialog(const QString &dir, QWidget *parent) : QDialog(parent), dir_(dir) {
    setWindowTitle(QStringLiteral("Uninstall OBCR"));
    setWindowIcon(QPixmap::fromImage(setup::icon(64)));
    resize(560, 280);
    auto *layout = new QVBoxLayout(this);
    text_ = paragraph(
        QStringLiteral("<p>This removes OBCR from <b>%1</b>: the game, its copy of your game files, "
                       "the shortcuts and the uninstaller. Your original game copy is not touched.</p>")
            .arg(QDir::toNativeSeparators(dir).toHtmlEscaped()),
        this);
    layout->addWidget(text_);
    QStringList user;
    for (const QString &d : setup::userDataDirs()) user << QDir::toNativeSeparators(d);
    purge_ = new QCheckBox(QStringLiteral("Also delete my OBCR settings and saved games (%1)")
                               .arg(user.join(QStringLiteral(", "))),
                           this);
    layout->addWidget(purge_);
    bar_ = new QProgressBar(this);
    bar_->setRange(0, 100);
    bar_->setVisible(false);
    layout->addWidget(bar_);
    layout->addStretch();
    auto *buttons = new QDialogButtonBox(this);
    go_ = buttons->addButton(QStringLiteral("&Uninstall"), QDialogButtonBox::AcceptRole);
    close_ = buttons->addButton(QDialogButtonBox::Cancel);
    layout->addWidget(buttons);
    connect(go_, &QPushButton::clicked, this, &UninstallDialog::run);
    connect(close_, &QPushButton::clicked, this, &QDialog::reject);
    if (!setup::isInstallation(dir)) {
        text_->setText(QStringLiteral("No OBCR installation was found in <b>%1</b>.")
                           .arg(QDir::toNativeSeparators(dir).toHtmlEscaped()));
        go_->setEnabled(false);
        purge_->setVisible(false);
    }
}

void UninstallDialog::run() {
    go_->setEnabled(false);
    close_->setEnabled(false);
    purge_->setEnabled(false);
    bar_->setVisible(true);
    const bool purge = purge_->isChecked();
    const QString dir = dir_;
    auto progress = [this](int percent, const QString &message) {
        QMetaObject::invokeMethod(
            this,
            [this, percent, message] {
                bar_->setValue(percent);
                text_->setText(message.toHtmlEscaped());
            },
            Qt::QueuedConnection);
    };
    QThread *thread = QThread::create([this, dir, purge, progress] {
        QString error;
        const bool ok = setup::uninstall(dir, purge, progress, &error);
        QMetaObject::invokeMethod(
            this,
            [this, ok, error] {
                text_->setText(ok ? QStringLiteral("<p><b>OBCR has been removed.</b></p>")
                                  : QStringLiteral("<p><b>Uninstall finished with problems.</b></p><p>%1</p>")
                                        .arg(error.toHtmlEscaped().replace(QLatin1Char('\n'),
                                                                           QStringLiteral("<br>"))));
                bar_->setValue(100);
                close_->setText(QStringLiteral("&Close"));
                close_->setEnabled(true);
                disconnect(close_, nullptr, this, nullptr);
                connect(close_, &QPushButton::clicked, this, ok ? &QDialog::accept : &QDialog::reject);
            },
            Qt::QueuedConnection);
    });
    connect(thread, &QThread::finished, thread, &QObject::deleteLater);
    thread->start();
}
