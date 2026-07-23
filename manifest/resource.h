// Resource IDs shared by app.rc and src/ui/dialog.rs.
// Keep these in sync with the `ids` module in src/ui/dialog.rs.

#define IDOK      1
#define IDCANCEL  2

// Dialog templates
#define IDD_SETTINGS  101
#define IDD_ADVANCED  102
#define IDD_PASSWORD  103

// --- Password dialog ---
#define IDC_PWINFO    3000
#define IDE_PW1       3001
#define IDE_PW2       3002
#define IDL_PW2       3004

// --- MVP dialog: edits ---
#define IDE_NAME          1001
#define IDE_HOSTNAME      1002
#define IDE_ADDRESSES     1003
#define IDE_USERNAME      1004
#define IDE_PASSWORD      1005
#define IDE_CERT          1006
#define IDE_COUNTRY       1007
#define IDE_REFRESH       1008
#define IDE_ENGINE        1009

// --- MVP dialog: combos ---
#define IDC_PROTOCOL      1101
#define IDC_FALLBACK      1102
#define IDC_RIR           1103
#define IDC_LOGLEVEL      1104

// --- MVP dialog: checks/buttons ---
#define IDC_SKIPVERIFY    1201
#define IDC_SPLIT         1202
#define IDC_KILLSWITCH    1203
#define IDC_IMPORT        1208
#define IDC_BROWSE        1209
#define IDC_ADVANCED      1210
#define IDC_CHANGEPW      1211

// --- Advanced dialog: edits ---
#define IDE_CLIENTRANDOM  2001
#define IDE_CUSTOMSNI     2002
#define IDE_MTU           2003
#define IDE_DNS           2004
#define IDE_KSPORTS       2005

// --- Advanced dialog: checks ---
#define IDC_IPV6          2101
#define IDC_ANTIDPI       2102
#define IDC_PQ            2103
#define IDC_CHANGEDNS     2104
