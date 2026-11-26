/* Ultra-fast (<1ms, ~1.5MB RSS) hardware presence fingerprint for Moondrop USB HID & BT.
 * Emits a deterministic fingerprint line so QML only invokes Python when devices change.
 */

#define _POSIX_C_SOURCE 200809L
#include <ctype.h>
#include <dirent.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static void str_lower(char *s) {
    for (; *s; ++s) *s = (char)tolower((unsigned char)*s);
}

static int is_moondrop_hid(const char *vid_hex, const char *name) {
    char low[512];
    snprintf(low, sizeof(low), "%s", name);
    str_lower(low);

    if (strcmp(vid_hex, "000031b2") == 0) {
        if (strstr(low, "chu2") || strstr(low, "chu 2") || strstr(low, "cdsp") ||
            strstr(low, "moondrop") || strstr(low, "quark") || strstr(low, "jiu") ||
            strstr(low, "may")) {
            return 1;
        }
    }
    if (strstr(low, "moondrop") || strstr(low, "rays") || strstr(low, "marigold") ||
        strstr(low, "freedsp") || strstr(low, "dawn pro") || strstr(low, "echo a") ||
        strstr(low, "moonriver") || strstr(low, "dha15")) {
        return 1;
    }
    return 0;
}

int main(void) {
    printf("usb=");
    DIR *hdir = opendir("/sys/class/hidraw");
    int first = 1;
    if (hdir) {
        struct dirent *ent;
        while ((ent = readdir(hdir)) != NULL) {
            if (strncmp(ent->d_name, "hidraw", 6) != 0) continue;
            char upath[320];
            snprintf(upath, sizeof(upath), "/sys/class/hidraw/%s/device/uevent", ent->d_name);
            FILE *f = fopen(upath, "r");
            if (!f) continue;

            char line[512], vid[32] = "", pid[32] = "", hname[512] = "";
            while (fgets(line, sizeof(line), f)) {
                size_t len = strlen(line);
                while (len > 0 && (line[len - 1] == '\n' || line[len - 1] == '\r')) line[--len] = '\0';
                if (strncmp(line, "HID_ID=", 7) == 0) {
                    char *p = line + 7;
                    char *c1 = strchr(p, ':');
                    char *c2 = c1 ? strchr(c1 + 1, ':') : NULL;
                    if (c1 && c2) {
                        *c2 = '\0';
                        snprintf(vid, sizeof(vid), "%s", c1 + 1);
                        snprintf(pid, sizeof(pid), "%s", c2 + 1);
                        str_lower(vid);
                        str_lower(pid);
                    }
                } else if (strncmp(line, "HID_NAME=", 9) == 0) {
                    snprintf(hname, sizeof(hname), "%s", line + 9);
                }
            }
            fclose(f);

            if (is_moondrop_hid(vid, hname)) {
                printf("%s%s:%s:%s", first ? "" : ",", ent->d_name, vid, pid);
                first = 0;
            }
        }
        closedir(hdir);
    }

    printf("|bt=");
    DIR *bdir = opendir("/sys/class/bluetooth");
    int bt_links = 0;
    if (bdir) {
        struct dirent *ent;
        while ((ent = readdir(bdir)) != NULL) {
            /* Connected Bluetooth links appear in /sys/class/bluetooth as hciX:Y */
            if (strncmp(ent->d_name, "hci", 3) == 0 && strchr(ent->d_name, ':') != NULL) {
                printf("%s%s", bt_links ? "," : "", ent->d_name);
                bt_links++;
            }
        }
        closedir(bdir);
    }
    printf("\n");
    return 0;
}
