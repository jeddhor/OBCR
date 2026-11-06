// OBCR Setup: the installation wizard and the uninstall dialog.
// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once

#include <QDialog>
#include <QWizard>
#include <QWizardPage>

#include "setup.h"

class QCheckBox;
class QLabel;
class QLineEdit;
class QPlainTextEdit;
class QProgressBar;
class QRadioButton;
class QTimer;
class QPushButton;

class SetupWizard;

class IntroPage : public QWizardPage {
    Q_OBJECT
public:
    explicit IntroPage(SetupWizard *w);
    bool validatePage() override;

private:
    SetupWizard *wizard_;
    QRadioButton *update_ = nullptr;
    QRadioButton *remove_ = nullptr;
};

class DataPage : public QWizardPage {
    Q_OBJECT
public:
    explicit DataPage(SetupWizard *w);
    void initializePage() override;
    bool isComplete() const override;
    void setDir(const QString &dir);

private:
    void check();
    SetupWizard *wizard_;
    QLineEdit *dir_;
    QLabel *status_;
    QTimer *debounce_;
};

class TargetPage : public QWizardPage {
    Q_OBJECT
public:
    explicit TargetPage(SetupWizard *w);
    void initializePage() override;
    bool isComplete() const override;
    bool validatePage() override;

private:
    void update();
    SetupWizard *wizard_;
    QLineEdit *dir_;
    QLabel *status_;
    QCheckBox *menu_;
    QCheckBox *desktop_;
    QCheckBox *pathLink_ = nullptr;
};

class InstallPage : public QWizardPage {
    Q_OBJECT
public:
    explicit InstallPage(SetupWizard *w);
    void initializePage() override;
    bool isComplete() const override;

private:
    SetupWizard *wizard_;
    QProgressBar *bar_;
    QLabel *status_;
    QPlainTextEdit *log_;
    bool done_ = false;
    bool ok_ = false;
};

class FinishPage : public QWizardPage {
    Q_OBJECT
public:
    explicit FinishPage(SetupWizard *w);
    void initializePage() override;
    bool startGame() const;

private:
    SetupWizard *wizard_;
    QLabel *text_;
    QCheckBox *start_;
};

class SetupWizard : public QWizard {
    Q_OBJECT
public:
    enum Page { Intro, Data, Target, Install, Finish };
    explicit SetupWizard(QWidget *parent = nullptr);
    void accept() override;

    setup::Options options;
    setup::DataCheck check;
    setup::Existing existing;
    DataPage *dataPage() const { return data_; }

private:
    DataPage *data_;
    FinishPage *finish_;
};

class UninstallDialog : public QDialog {
    Q_OBJECT
public:
    explicit UninstallDialog(const QString &dir, QWidget *parent = nullptr);

private:
    void run();
    QString dir_;
    QLabel *text_;
    QCheckBox *purge_;
    QProgressBar *bar_;
    QPushButton *go_;
    QPushButton *close_;
};
