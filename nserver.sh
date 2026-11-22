#!/bin/bash
# 各种服务端管理工具


nasyt_dir="$HOME/.nserver"
version="0.7"
time_date="2026/8/12"

source $nasyt_dir/config.txt

# 检查包管理器的函数
check_pkg_install() {
    if [ -f /etc/os-release ]; then
        source /etc/os-release #加载变量
    fi
    if [[ -n $TERMUX_VERSION ]]; then
        sys="(Termux 终端)"
        PRETTY_NAME="Termux"
        #sed -i 's@^\(deb.*stable main\)$@#\1\ndeb https://mirrors.tuna.tsinghua.edu.cn/termux/termux-packages-24 stable main@' $PREFIX/etc/apt/sources.list >/dev/null
        pkg_install="pkg install"
        pkg_remove="pkg remove"
        pkg_update="pkg update"
        deb_sys="pkg"
        yes_tg="-y"
        deb_size="$(dpkg -l | wc -l)" >/dev/null 2>&1
        
    elif command -v apt-get >/dev/null 2>&1; then
        sys="(Debian/Ubuntu 系列)"
        pkg_install="apt install"
        pkg_remove="apt remove"
        pkg_update="apt update"
        sudo_setup="sudo"
        deb_sys="apt"
        yes_tg="-y"
        deb_size="$(dpkg -l | wc -l)" >/dev/null 2>&1
        
    elif command -v dnf >/dev/null 2>&1; then
        sys="(Fedora/RHEL/CentOS 8 及更高版本)"
        pkg_install="dnf install"
        pkg_remove="dnf remove"
        pkg_update="dnf update"
        sudo_setup="sudo"
        deb_sys="dnf"
        yes_tg="-y"
        deb_size="$(rpm -qa | wc -l)" >/dev/null 2>&1
        
    elif command -v yum >/dev/null 2>&1; then
        sys="(Fedora/RHEL/Rocky/CentOS 7 及更早版本)"
        pkg_install="yum install"
        pkg_remove="yum remove"
        pkg_update="yum update"
        sudo_setup="sudo"
        deb_sys="yum"
        yes_tg="-y"
        deb_size="$(rpm -qa | wc -l)" >/dev/null 2>&1
        
    elif command -v pacman >/dev/null 2>&1; then
        sys="(Arch Linux 系列)"
        pkg_install="pacman -S"
        pkg_remove="pacman -R"
        pkg_update="pacman -Syu"
        sudo_setup="sudo"
        deb_sys="pacman"
        yes_tg="-y"
        deb_size="$(pacman -Q | wc -l)" >/dev/null 2>&1
        
    elif command -v zypper >/dev/null 2>&1; then
        sys="(openSUSE 系列)"
        pkg_install="zypper in -y"
        pkg_remove="zypper rm"
        sudo_setup="sudo"
        deb_sys="zypper"
        yes_tg="-y"
        deb_size="$(zypper se -i | wc -l)" >/dev/null 2>&1
        
    elif command -v apk >/dev/null 2>&1; then
        sys="(Alpine/PostmarketOS系统)"
        sed -i 's#https\?://dl-cdn.alpinelinux.org/alpine#https://mirrors.tuna.tsinghua.edu.cn/alpine#g' /etc/apk/repositories
        pkg_install="apk add"
        pkg_remove="apk del"
        pkg_update="apk update"
        sudo_setup="sudo"
        deb_sys="apk"
        yes_tg=""
        deb_size="$(apk info | wc -l)" >/dev/null 2>&1
        
    elif command -v emerge >/dev/null 2>&1; then
        sys="(gentoo/funtoo 系统)"
        pkg_install="emerge -avk"
        pkg_remove="emerge -C"
        sudo_setup="sudo"
        deb_sys="emerge"
        yes_tg="-y"
        deb_size="$(qlist -I | wc -l)" >/dev/null 2>&1
        
    elif [[ "$(uname -s)" == "Darwin" ]]; then
        brew_install #brew安装检测
        sys="(MacOS 系统)"
        pkg_install="brew install"
        sudo_setup="sudo"
        deb_sys="brew"
        yes_tg="-y"
        deb_size="$(brew list | wc -l)" >/dev/null 2>&1
        read -p "抱歉，目前没有完全适配MacOS系统"
        
    else
        echo -e "$(info) >_<未检测到支持的系统。"
    fi
}

# 用户协议
eula() {
    [[ -n $eula ]] && return 0
    if [[ $language == "China" ]]; then
        $habit --title "免责声明" \
        --yesno "本工具使用带来的后果由使用者承担全部责任\n运行所造成的损失作者依不承担\n你是否同意使用条款？" 0 0
    else
        $habit --title "Disclaimer" \
        --yesno "The user bears full responsibility for the consequences of using this tool.\nThe author does not assume any liability for any losses caused by its operation.\nDo you agree to the terms of use?" 0 0
    fi
    if [ $? -eq 0 ]; then
        config add eula true
        return 0
    else
        $habit --msgbox "未同意使用条款,脚本退出。" 0 0
        rm $bin/nserver
        rm $nasyt_dir/config.txt
        clear
        exit 0
    fi
}

# 常用变量
usevar() {
    [[ -n $PREFIX ]] && bin=$PREFIX/bin || bin=/usr/bin
    [[ -z $open ]] && config add open micro
    if [[ -z $update ]]; then
        $habit --title "自动检测更新" --yesno "是否自动检测工具更新?\n可能会拖延启动时间" 0 0
        [[ $? -eq 0 ]] && config add update true || config add update false
    fi
    br() {
        echo -e "$blue----------------------------$color"
    }
}

# 颜色输出菜单
color_menu() {
    while true
    do
        color_menu_xz=$($habit --clear --title "颜色输出设置" \
        --menu "当前设置:$(config load color_var)\n请选择" 0 0 10 \
        true "开启颜色输出功能" \
        false "关闭颜色输出功能" \
        0 "◀返回" \
        2>&1 1>/dev/tty)
        case $color_menu_xz in
            true)
                config add color_var true
                ;;
            false)
                config add color_var false
                ;;
            *)
                break
                ;;
        esac
        $habit --msgbox "设置完成" 0 0
    done
}
color_variable() {
    if [[ -z $color_var ]]; then
        config add color_var true
    fi
    color_var_1=$(config load color_var)
    if [[ $color_var_1 == "false" ]]; then
        color=''
        green=''
        blue=''
        red=''
        yellow=''
        grey=''
        pink=''
        cyan=''
    else
        color='\033[0m'
        green='\033[0;32m'
        blue='\033[0;34m'
        red='\033[31m'
        yellow='\033[33m'
        grey='\e[37m'
        pink='\033[38;5;218m'
        cyan='\033[96m'
    fi
}

all_variable() {
    OUTPUT_FILE="nasyt" # 下载文件名
    time_out=5  # curl超时时间（秒）
    urls=(
      "https://nasyt.hoha.top/shell/nasyt.sh"
      "https://raw.githubusercontent.com/nasyt233/nasyt-linux-tool/refs/heads/master/nasyt.sh"
      "https://ghfast.top/https://raw.githubusercontent.com/nasyt233/nasyt-linux-tool/refs/heads/master/nasyt.sh"
      "https://nasyt2.class2.icu/shell/nasyt.sh"
    )
    
}


# 函数
default_habit() {
    [[ -z $habit ]] && habit_xz
}

server_ip() {
    server_ip=$(hostname -i) # 服务器IP
    $habit --msgbox "当前IP为: $server_ip" 0 0
}

info() {
    echo -e "$cyan[$(date +"%r")]$color $green[INFO]$color" $*
}

uptime_cn() {
    uptime_sc=$(uptime -p | sed 's/up/运行/; s/week/周/; s/days/天/; s/day/天/; s/hours/小时/; s/hour/小时/; s/minutes/分钟/; s/minute/分钟/; s/users/用户/; s/user/用户/; s/load average/平均负载/')
    
}


esc() {
    if [[ $language == "China" ]]; then
        echo -e "$(info) 按$green回车键$color$blue返回$color,按$yellow Ctrl+C$color$red退出$color"
    elif [[ $language == "English" ]]; then
        echo -e "$(info) Press $green Enter$color$blue to return$color, press $yellow Ctrl C$color$red to exit$color"
    else
        echo -e "$(info) Press $green Enter$color$blue to return$color, press $yellow Ctrl C$color$red to exit$color"
    fi
    read
}

bash_url() {
    bash -c "$(curl -fsL $@)"
    if [ $? -ne 0 ]; then
        echo -e "$(info) $red 拉取脚本失败$color"
    fi
}

x() {
    tar -xzvf $1 -C $2 >/dev/null 2>&1
    cw_test=$?
    if [ $cw_test -ne 0 ]; then
        echo -e "$(info) $red ❌ 错误代码: $cw_test $color"
    else
        echo -e "$(info) $green 文件解压成功$color"
    fi
}

#国内外检测
country() {
    country=$(curl -s https://myip.ipip.net | grep -oE "中国|China" 2>/dev/null)
    if [ -n "$country" ]; then
        github_speed="https://gh-proxy.com"
        config add language China
    else
        github_speed=""
        config add language English
    fi
}


habit_menu () {
    clear
    if [[ $language == "China" ]]; then
        echo "功能都支持使用箭头进行选择"
        br
        echo "1) dialog屏幕点击(适合鼠标)"
        echo "2) whiptail屏幕滑动（适合触屏)"
        echo "3) 重置选择"
        br
        read -p "请选择菜单使用习惯: " habit_menu_xz
    else
        echo "All functions support selection using the arrow keys"
        br
        echo "1) Click on the dialog screen (suitable for mouse)"
        echo "2) Slide on the whiptail screen (suitable for touchscreen)"
        echo "3) Reset selection"
        br
        read -p "Please choose your menu usage preference: " habit_menu_xz
    fi
}
habit_xz() {
    habit_menu
    case $habit_menu_xz in
       1)
           test_install dialog
           config add habit dialog
           ;;
       2) 
           test_whiptail
           config add habit whiptail
           ;;
       3) config clear ;;
       *) break ;;
    esac
    test_install $habit
}

#默认打开设置
default_open() {   
    while true
    do
        default_open_xz=$($habit --clear --title "默认打开设置" \
        --menu "当前方式：$(config load open)\n请选择" 0 0 10 \
        1 "Micro" \
        2 "Nano" \
        3 "Vim/Nvim" \
        4 "Emacs" \
        5 "Helix" \
        6 "自行设置" \
        0 "◀返回" \
        2>&1 1>/dev/tty)
        case $default_open_xz in
            1)
                test_install micro
                config add open micro
                esc
                ;;
            2)
                test_install nano
                config add open nano
                ;;
            3)
                test_install vim
                if command -v nvim >/dev/null 2>&1; then
                    config add open nvim
                else
                    config add open vim
                fi
                ;;
            4)
                test_install emacs
                config add open emacs
                ;;
            5)
                test_install helix
                config add open helix
                ;;
            6)
                default_open_diy=$($habit --clear --title "自定义" \
                --inputbox "请输入已安装的软件包名字" 0 0 \
                2>&1 1>/dev/tty)
                config add open $default_open_diy
                ;;
            *)
                break
                ;;
        esac
        echo -e "$(info) $green 设置完成$color"
    done
}

#github加速工具
github_speed_tool() {
    github_speed_menu() {
        github_speed_xz=$($habit --clear --title "github加速设置" \
        --menu "当前加速地址: $github_speed \n请选择一个github加速地址" 0 0 10 \
        1 "gh-proxy.com" \
        2 "ghfast.top" \
        3 "ghproxy.net" \
        4 "ghproxy.homeboyc.cn" \
        5 "gh.jasonzeng.dev" \
        6 "github.xxlab.tech" \
        8 "不使用加速" \
        9 "自定义链接" \
        0 "◀返回" \
        2>&1 1>/dev/tty)
    }
    github_speed_while() {
        github_speed_menu
        case $github_speed_xz in
            1)
                github_speed_sz="https://gh-proxy.com/"
                ;;
            2)
                github_speed_sz="https://ghfast.top/"
                ;;
            3)
                github_speed_sz="https://ghproxy.net/"
                ;;
            4)
                github_speed_sz="https://ghproxy.homeboyc.cn/"
                ;;
            5)
                github_speed_sz="https://gh.jasonzeng.dev/"
                ;;
            6)
                github_speed_sz="https://github.xxlab.tech/"
                ;;
            8)
                config add github_speed ""
                $habit --msgbox "重置完成" 0 0
                ;;
            9)
                github_speed_sz=$($habit --clear --title "自定义设置加速链接" \
                --inputbox "请输入链接(例如https://xxx.com)\n加速链接要用/结尾" 0 0 \
                2>&1 1>/dev/tty)
                [[ $? -ne 0 ]] && break
                ;;
            *)
                break
                ;;
        esac
        config add github_speed $github_speed_sz
        $habit --msgbox "设置完成，如需切换请在脚本设置进行切换" 0 0
    }
    if ! [[ -v github_speed ]]; then
        github_speed_while
    fi
}

# SHA256校验通用函数
sha() {
    local sha_file="$1"
    local expect="$2"

    if [ ! -f "$sha_file" ]; then
        echo -e "$(info) $red 文件不存在 $color"
        return 1
    fi

    local local_sum
    if command -v sha256sum >/dev/null 2>&1; then
        local_sum=$(sha256sum "$sha_file")
    elif command -v sha256 >/dev/null 2>&1; then
        local_sum=$(sha256 "$sha_file")
    elif command -v shasum >/dev/null 2>&1; then
        local_sum=$(shasum -a 256 "$sha_file")
    else
        echo -e "$(info) $red 系统无可用的SHA256计算工具 $color"
        return 2
    fi

    if [[ "$local_sum" == "$expect" ]]; then
        echo -e "$(info) $green SHA256 校验通过$color"
        return 0
    else
        echo -e "$(info) $red SHA256 校验不匹配 $color"
        echo "本地: $local_sum"
        echo "官方: $expect"
        return 1
    fi
}

# 网络工具使用限制检查
disclaimer() {
    if [[ -f "$nasyt_dir/disclaimer" ]]; then
        return 0
    fi
    if [[ $language == "China" ]]; then
        $habit --title "免责声明" \
        --yesno "本工具仅限合法使用\n带来的后果由使用者承担全部责任\n运行所造成的损失作者依不承担\n你是否同意使用条款？" 0 0
    else
        $habit --title "Disclaimer" \
        --yesno "This tool is for legal use only\nUsers are fully responsible for the consequences\nThe author is not responsible for any losses caused by running it\nDo you agree to the terms of use?" 0 0
    fi
    if [ $? -eq 0 ]; then
        touch "$nasyt_dir/disclaimer"
        return 0
    else
        $habit --msgbox "未同意使用条款,脚本退出。" 0 0
        clear
        exit 0
    fi
}

#MacOS_brew软件包安装。
brew_install() {
    if command -v brew >/dev/null 2>&1; then
        echo "break已安装"
    else
        xcode-select --install
        bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
    fi

}


# 配置管理函数
config() {
    local config_file=${4:-$nasyt_dir/config.txt} >/dev/null 2>&1
    mkdir -p "$(dirname "$config_file")" >/dev/null 2>&1
    [ ! -f "$config_file" ] && touch "$config_file"

    config_add() {
        local key="$1"
        local val="$2"
        val="${val//\"/\\\"}"
        sed -i "/^${key}=/d" "$config_file"
        echo "${key}=${val}" >> "$config_file"
    }

    config_load() {
        local key="$1"
        local res
        res=$(awk -v k="$key" '$0 ~ "^"k"=" {sub(/^[^=]+=/,""); print}' "$config_file")
        [[ -z "$res" ]] && return 1
        echo "$res"
    }

    config_del() {
        local key="$1"
        sed -i "/^${key}=/d" "$config_file"
    }

    config_list() {
        cat "$config_file"
    }

    config_clear() {
        > "$config_file"
    }

    case "$1" in
        a|add) shift; config_add "$@" ;;
        load) shift; config_load "$@" ;;
        d|del)  shift; config_del "$@"  ;;
        l|list) config_list ;;
        c|clear) config_clear ;;
    esac
    #重载配置文件
    source "$config_file" >/dev/null 2>&1
}

#yml配置文件管理
yml() {

    _yq_is_go() {
        if yq --version 2>&1 | grep -qi "mikefarah"; then
            return 0
        fi
        return 1
    }
    _yq_eval() {
        local args=()
        local expr file
        local has_inplace=0

        # 遍历参数判断是否包含 -i
        for arg in "$@"; do
            if [[ "$arg" == "-i" ]]; then
                has_inplace=1
            fi
        done

        if _yq_is_go; then
            # Go-yq 语法：yq eval [opts] expr file
            args+=("eval")
            # 把传入的额外参数（如 -i）放前面
            while [[ $# -ge 3 ]]; do
                args+=("$1")
                shift
            done
            expr="$1"
            file="$2"
            yq "${args[@]}" "$expr" "$file"
        else
            # Python-yq 语法：yq [opts] expr file
            while [[ $# -ge 3 ]]; do
                args+=("$1")
                shift
            done
            expr="$1"
            file="$2"
            # python-yq 带 -i 必须加 -y
            if [[ $has_inplace -eq 1 ]]; then
                args+=("-y")
            fi
            yq "${args[@]}" "$expr" "$file"
        fi
    }

    #检测文件
    yml_file() {
        [ -f "$1" ]||{ echo -e "$info $red 错误：没有$1 文件$color";return 1;}
    }

    #读取
    yml_load() {
        yml_file "$1" || return 1
        _yq_eval -r ".$2" "$1"
    }

    #新增/覆盖val
    yml_add(){
        [ ! -f "$1" ] && touch "$1"
        _yq_eval -i ".$2 = \"$3\"" "$1"
    }

    #删除
    yml_del() {
        yml_file "$1" || return 1
        _yq_eval -i "del(.$2)" "$1"
    }

    #清空
    yml_clear() {
        > "$1"
    }

    #列出
    yml_list() {
        yml_file "$1" || return 1
        cat "$1"
    }
    
    command -v yq >/dev/null 2>&1 || test_install jq

    case "$1" in
        add) shift; yml_add "$@" ;;
        load) shift; yml_load "$@" ;;
        del)  shift; yml_del "$@"  ;;
        list) shift; yml_list "$@" ;;
        clear) shift; yml_clear "$@" ;;
    esac
}


#文件夹选择
dir() {
    dir_outvar="$2"
    dir_list=$(ls -d "$1"/*/ 2>/dev/null | sed -E 's|.*/([^/]+)/$|\1|' | sort -V)
    eval "$dir_outvar=($(printf '"%s" ' $dir_list))"
    #dir_xz=${a[@]}
    #echo ${a[@]}
}



#文件选择器
file_xz() {
    #处理
    file_browser_xz() {
        #第一个目录参数
        current_dir="${1:-.}"
        #第二个变量参数
        file_var="${2:-file_index}"
        
        # 检查目录是否存在
        if [[ ! -d "$current_dir" ]]; then
            echo "目标目录 '$current_dir' 不存在" >&2
            return 1
        fi
            #循环
            while true
            do
                local menu_items=()
                
                #如果不是根目录，添加返回选项
                if [[ "$current_dir" != "." ]]; then
                    menu_items+=(".." "📁 ◀返回上级目录 Back to Previous Directory")
                fi
                
                #添加当前目录内容
                if [[ -v 3 ]]; then
                    while IFS= read -r item; do
                        if [[ -n "$item" ]]; then
                            if [[ -d "$current_dir/$item" ]]; then
                                menu_items+=("$item" "📁 $item/")
                            else
                                menu_items+=("$item" "📄 $item")
                            fi
                        fi
                    done < <(ls -a "$current_dir" --group-directories-first | grep -E ".*.$3\$")
                else
                    while IFS= read -r item; do
                        if [[ -n "$item" ]]; then
                            if [[ -d "$current_dir/$item" ]]; then
                                menu_items+=("$item" "📁 $item/")
                            else
                                menu_items+=("$item" "📄 $item")
                            fi
                        fi
                    done < <(ls -a "$current_dir" --group-directories-first )
                fi
                dir_xz=$($habit --title "文件选择器/File Explorer" \
                --menu "文件浏览器/File Explorer: $current_dir 🤓👇" 0 0 15 \
                "${menu_items[@]}" \
                2>&1 1>/dev/tty)
                cw_test=$?
                if [[ -z "$dir_xz" ]]; then
                    break
                fi
                
                if [[ "$dir_xz" == ".." ]]; then
                    current_dir=$(dirname "$current_dir")
                elif [[ -d "$current_dir/$dir_xz" ]]; then
                    current_dir="$current_dir/$dir_xz"
                else
                    $habit --yesno "确认文件 Confirmation document: $current_dir/$dir_xz" 0 0
                    if [ $? -eq 0 ]; then
                        eval "$file_var"="$current_dir/$dir_xz"
                        break
                    fi
                fi
            done    
        }
    file_browser_xz "$@"
    #输出
    #if [[ -n $file_index ]]; then
    #    echo $file_index
    #else
    #    echo $file_var
    #fi
}

# tmux管理函数
tmux_tool() {
    test_install tmux
    tmux_get() {
        tmux_ls=$(mktemp)
        tmux list-sessions -F "#S" 2>/dev/null > "$tmux_ls"
    }

    tmux_clean() {
        [[ -n "$tmux_ls" && -f "$tmux_ls" ]] && rm -f "$tmux_ls"
    }

    tmux_menu() {
        $habit --clear \
        --title "tmux工具" \
        --menu "请选择操作" 0 0 10 \
        1 "🆕 新建会话" \
        2 "🔧 会话管理" \
        3 "❓ 使用说明" \
        4 "🌌 OhMyTmux美化配置" \
        0 "◀ 退出" \
        2>&1 1>/dev/tty
    }

    while true; do
        tmux_menu_xz=$(tmux_menu)
        [[ -z "$tmux_menu_xz" ]] && break

        case "$tmux_menu_xz" in
            1)
                tmux_new=$($habit --clear --title "🆕新建会话" \
                    --inputbox "输入会话名称：" 0 0 "" \
                    2>&1 1>/dev/tty)
                if [[ -z "$tmux_new" ]]; then
                    continue
                fi
                tmux new-session -d -s "$tmux_new"
                $habit --title "操作确认" --yesno "成功创建会话：$tmux_new\n是否直接接入会话？" 0 0
                if [[ $? -eq 0 ]]; then
                    tmux attach-session -t "$tmux_new"
                fi
                ;;

            2)
                while true; do
                    tmux_get
                    if [[ ! -s "$tmux_ls" ]]; then
                        $habit --title "警告" --msgbox "暂无运行中的tmux会话！\n请先创建会话" 10 35
                        tmux_clean
                        break
                    fi

                    local tmux_list=()
                    local tmux_number=1
                    declare -A session_map
                    while IFS= read -r session; do
                        [[ -z "$session" ]] && continue
                        tmux_list+=("$tmux_number" "$session")
                        session_map["$tmux_number"]="$session"
                        ((tmux_number++))
                    done < "$tmux_ls"
                    tmux_list+=("0" "◀ 返回上级")

                    tmux_list_xz=$($habit --clear \
                        --title "会话管理" \
                        --menu "选择需要操作的会话：" 0 0 10 \
                        "${tmux_list[@]}" \
                        2>&1 1>/dev/tty)
                    tmux_clean

                    if [[ "$tmux_list_xz" == "0" || -z "$tmux_list_xz" ]]; then
                        unset session_map
                        break
                    fi

                    local tmux_name="${session_map[$tmux_list_xz]}"
                    unset session_map

                    tmux_session_menu=$($habit --title "[$tmux_name] 操作面板" \
                    --menu "请选择：" 0 0 10 \
                    1 "🔗 接入会话" \
                    2 "🔴 彻底关闭会话" \
                    3 "📝 重命名会话" \
                    0 "◀ 返回列表" \
                    2>&1 1>/dev/tty)

                    case "$tmux_session_menu" in
                        1)
                            if [[ -n "$TMUX" ]]; then
                                tmux switch-client -t "$tmux_name"
                            else
                                tmux attach -t "$tmux_name"
                            fi
                            esc
                            ;;
                        2)
                            $habit --title "危险确认" --yesno "确定要销毁会话【$tmux_name】？\n会话内所有进程将会终止！" 10 40
                            if [[ $? -eq 0 ]]; then
                                tmux kill-session -t "$tmux_name"
                                $habit --msgbox "✅ 会话 $tmux_name 已关闭" 0 0
                            fi
                            ;;
                        3)
                            tmux_new_name=$($habit --title "重命名会话" \
                                --inputbox "原会话名：$tmux_name\n输入新名称：" \
                                0 0 "$tmux_name" \
                                2>&1 1>/dev/tty)
                            if [[ -n "$tmux_new_name" ]]; then
                                if tmux rename-session -t "$tmux_name" "$tmux_new_name"; then
                                    $habit --msgbox "✅ 重命名成功\n$tmux_name → $tmux_new_name" 0 0
                                fi
                            fi
                            ;;
                        *)
                            ;;
                    esac
                done
                ;;

            3)
                $habit --msgbox '【Tmux简易操作指南】
本工具由 NAS油条 制作

常用快捷键（进入会话内生效）：
• Ctrl+b %    垂直分割窗格
• Ctrl+b "    水平分割窗格
• Ctrl+b 方向键 切换窗格
• Ctrl+b c    新建窗口
• Ctrl+b n/p  前后切换窗口
• Ctrl+b d    脱离会话（后台运行）
• Ctrl+b ,    重命名当前窗口
' 0 0
                ;;

            4)
                test_install git
                cd "$HOME" || return
                echo -e "$info $blue 正在部署 Oh My Tmux 配置 $color"

                if [[ -d "$HOME/.tmux" ]]; then
                    echo -e "$info $yellow 仓库目录已存在，跳过克隆 $color"
                else
                    git clone --single-branch https://gitcode.com/gh_mirrors/tm/.tmux.git
                    local cw_test=$?
                    if [[ $cw_test -ne 0 ]]; then
                        echo -e "$info $red 仓库克隆失败，请检查网络！ $color"
                        esc
                        break
                    fi
                    echo -e "$info $green 仓库克隆完成 $color"
                fi
                ln -sf .tmux/.tmux.conf "$HOME/.tmux.conf"
                cp -n .tmux/.tmux.conf.local "$HOME/"
                echo -e "$info $green ✅ tmux美化配置部署完成！提示：重启tmux生效，或在会话内执行：\ntmux source ~/.tmux.conf $color"
                esc
                ;;

            0)
                break
                ;;
        esac
    done
}

cpolar_tool() {
    cpolar_tips() {
        $habit --msgbox "已启动服务" 0 0
    }
    while true
    do
        if ! command -v cpolar >/dev/null 2>&1; then
            if [[ -n $PREFIX ]]; then
                mkdir -p $PREFIX/etc/apt/sources.list.d
                echo "deb [trusted=yes] http://termux.cpolar.com termux extras" >> $PREFIX/etc/apt/sources.list.d/cpolar.list
                pkg update;pkg install cpolar
            else
                curl --progress-bar -L https://www.cpolar.com/static/downloads/install-release-cpolar.sh | sudo bash
            fi
        else
            cpolar_menu_xz=$($habit --clear --title "cpolar管理" \
            --menu "官网地址: www.cpolar.com\n密钥获取:dashboard.cpolar.com/auth \n默认启动为前台启动,推荐搭配tmux使用\n请选择" 0 0 10 \
            1 "密钥验证登录" \
            2 "http 开网站" \
            3 "tcp 联机" \
            4 "ftp 传文件" \
            8 "tmux后台管理工具" \
            9 "卸载本工具" \
            0 "◀返回" \
            2>&1 1>/dev/tty)
            case $cpolar_menu_xz in
                1)
                    cpolar_key=$($habit --clear --title "密钥输入" \
                    --inputbox "密钥获取:dashboard.cpolar.com/auth\n请输入密钥:" 0 0 \
                    2>&1 1>/dev/tty)
                    cpolar authtoken $cpolar_key
                    $habit --msgbox "验证完成" 0 0
                    ;;
                2)
                    cpolar_http=$($habit --clear --title "端口输入" \
                    --inputbox "请输入端口" 0 0 \
                    2>&1 1>/dev/tty)
                    cpolar http $cpolar_http
                    ;;
                3)
                    cpolar_tcp=$($habit --clear --title "端口输入" \
                    --inputbox "请输入端口" 0 0 \
                    2>&1 1>/dev/tty)
                    cpolar http $cpolar_tcp
                    ;;
                4)
                    cpolar_ftp=$($habit --clear --title "端口输入" \
                    --inputbox "请输入端口" 0 0 \
                    2>&1 1>/dev/tty)
                    cpolar http $cpolar_ftp
                    ;;
                8)
                    tmux_tool
                    ;;
                9)
                    pkg_remove cpolar
                    $habit --msgbox "卸载完成" 0 0
                    ;;
                *)
                    break
                    ;;
            esac
        fi
    done
}

#监控服务器资源
resources_show() {
    echo -e "$(info) 正在读取数据中"
    if [[ -n $TERMUX_VERSION ]]; then
        resources_show_notermux="CPU 使用率：不支持termux"
    else
        cpu_usage=$(grep 'cpu ' /proc/stat | awk '{u=$2+$4; t=$2+$4+$5; print "" sprintf("%.1f%%", u/t*100)}') >/dev/null 2>&1
        resources_show_notermux="CPU 使用率：$cpu_usage%"
        cpu_core=grep 'cpu[0-9]' /proc/stat | awk '{u=$2+$4; t=$2+$4+$5; printf "CPU核心%s：%.1f%%\n", substr($1,4), u/t*100}'
    fi
    #cpu_usage=$(top -bn1 | grep "Cpu(s)" | sed "s/.*,*([-9.)* id.*/\1/" | awk '{print 100}' >/dev/null 2>&1)
    mem_total=$(grep MemTotal /proc/meminfo | awk '{printf "%.1fGiB", $2/1024/1024}'); >/dev/null 2>&1
    mem_available=$(grep MemAvailable /proc/meminfo | awk '{printf "%.1fGiB", $2/1024/1024}'); >/dev/null 2>&1
    mem_usage=$(free | awk '/Mem/ {print $3/$2*100.0}') >/dev/null 2>&1
    #mem_used=$(grep MemTotal /proc/meminfo | awk '{t=$2} END {grep MemAvailable /proc/meminfo | awk -v t=t "{printf \"%.1fGiB\", (t-$2)/1024/1024}"}') >/dev/null 2>&1
    swap_total=$(grep SwapTotal /proc/meminfo | awk '{if($2==0){print "0.0GiB"}else{printf "%.1fGiB", $2/1024/1024}}'); >/dev/null 2>&1
    swap_free=$(grep SwapFree /proc/meminfo | awk '{if($2==0){print "0.0GiB"}else{printf "%.1fGiB", $2/1024/1024}}'); >/dev/null 2>&1
    #swap_used=$(grep SwapTotal /proc/meminfo | awk '{t=$2} END {grep SwapFree /proc/meminfo | awk -v t=t "{if(t==0){print \"0.0GiB\"}else{printf \"%.1fGiB\", (t-$2)/1024/1024}}"}'); >/dev/null 2>&1
    ps_quantity=$(ps -e --no-headers | wc -l) >/dev/null 2>&1
    swap_usage=$(grep -E 'SwapTotal|SwapFree' /proc/meminfo | awk -v total=$(grep SwapTotal /proc/meminfo | awk '{print $2}') '{if($1=="SwapFree:"){if(total==0){printf "利用率：0.0%%\n"}else{printf "利用率：%.1f%%\n", (total-$2)/total*100}}}') >/dev/null 2>&1
    echo -e "$(info) $green 读取数据完毕$color"
    $habit --msgbox "操作系统: $PRETTY_NAME \n\n$resources_show_notermux \n    $cpu_core\n内存总量：$mem_total 使用率：$mem_usage%\n    可用：$mem_available  \n\nSwap总量：$swap_total $swap_usage\n    可用：$swap_free \n\n进程数量: $ps_quantity" 0 0
}

# 脚本信息
shell_info() {
    if [[ $language == "China" ]]; then
        echo "工具名称: nserver"
        echo "当前版本: $version"
        echo "更新时间: $time_date"
        echo "操作系统: $PRETTY_NAME"
        echo "终端类型: $(basename $SHELL)"
        echo "工具语言: $language"
        echo "位于目录: $(command -v nasyt)"
        echo "当前时间: $(date +"%Y-%m-%d %H:%M.%S")"
        echo "运行时间: $(uptime_cn;echo $uptime_sc)"
        echo "软件包数: 共 $deb_size 个软件包"
        echo "内存剩余: $(grep MemAvailable /proc/meminfo | awk '{printf "%.1fGiB", $2/1024/1024}')"
        echo "进程数量: $(ps -e --no-headers | wc -l)"
    else
        echo "Tool Name: nserver"
        echo "Current Version: $version"
        echo "Update Time: $time_date"
        echo "Operating System: $PRETTY_NAME"
        echo "Terminal Type: $(basename $SHELL)"
        echo "Tool Language: $language"
        echo "Located Directory: $(command -v nasyt)"
        echo "Current Time: $(date  "%Y-%m-%d %H:%M.%S")"
        echo "Uptime: $(uptime_cn;echo $uptime_sc)"
        echo "Number of Packages: Total $deb_size packages"
        echo "Available Memory: $(grep MemAvailable /proc/meminfo | awk '{printf "%.1fGiB", $2/1024/1024}')"
        echo "Number of Processes: $(ps -e --no-headers | wc -l)"
    fi
}

# 根据时间返回问候语
get_greeting() {
    local hour=$(date +"%H")
    if [[ -n $TERMUX_VERSION ]]; then
        get_sys=Termux
    else
        get_sys=Linux
    fi
    if [[ $language == "China" ]]; then
        case $hour in
            05|06|07|08|09|10|11)
                echo "🌅 早上好！欢迎使用$get_sys工具箱"
                ;;
            12|13|14|15|16|17|18)
                echo "☀️ 下午好！欢迎使用$get_sys工具箱"
                ;;
            *)
                echo "🌙 晚上好！欢迎使用$get_sys工具箱"
                ;;
        esac
    else
        case $hour in
            05|06|07|08|09|10|11)
                echo "🌅 Good morning! Welcome to use the $get_sys toolbox"
                ;;
            12|13|14|15|16|17|18)
                echo "☀️ Good afternoon! Welcome to use the $get_sys toolbox"
                ;;
            *)
                echo "🌙 Good evening! Welcome to use the $get_sys toolbox"
                ;;
        esac
    fi
}

test_termux() {
    if [[ -n $TERMUX_VERSION ]]; then
        if [[ $language == "China" ]]; then
            $habit --msgbox "不支持Termux终端" 0 0
        else
            $habit --msgbox "Termux terminal not supported" 0 0
        fi
        break
    fi
}


#类figlet
test_toilet() {
    if command -v toilet >/dev/null 2>&1; then
        echo -e "$green ◉ toilet已安装，跳过安装步骤 $color"
    else
        echo "$(info) toilet未安装，正在安装"
        $pkg_install toilet $yes_tg
    fi
}

test_whiptail() {
    if command -v whiptail >/dev/null 2>&1; then
        echo -e "$(info) ◉ whiptail已安装, 跳过安装步骤。"
    else
        echo -e "$(info) whiptail未安装，正在安装。"
        if command -v pacman >/dev/null 2>&1; then
            echo -e "$(info) 检测到pacman软件包系统，正在安装libnewt软件包"
            test_install libnewt
        elif command -v dnf >/dev/null 2>&1; then
            echo -e "$(info) 检测到dnf软件包管理系统，正在安装newt软件包"
            test_install newt
        elif command -v apk >/dev/null 2>&1; then
            echo -e "$(info) 检测到apk软件包管理系统，正在安装newt软件包"
            test_install newt
        else
            echo -e "$(info) 正在使用 $deb_sys 安装whiptail"
            test_install whiptail newt
        fi
    fi
}

# python安装
test_python() {
    if [[ -z $PREFIX ]]; then
        test_install python3
        if ! command -v python >/dev/null 2>&1; then
            test_install python-is-python3
        fi
    else
        test_install python
    fi
}

test_eatmydata() {
    if command -v eatmydata >/dev/null 2>&1; then
        echo -e "$green ◉ eatmydata已安装,跳过安装$color"
    else
        if [[ -e /etc/os-release ]]; then
            echo -e "$(info) 正在安装eatmydata"
            if command -v apk >/dev/null 2>&1; then
                test_install libeatmydata
            elif command -v pacman >/dev/null 2>&1; then
                test_install libeatmydata
            else
                test_install eatmydata
            fi
        fi
    fi
}

test_hashcat() {
    if command -v hashcat >/dev/null 2>&1; then
        echo -e "$green ◉ hashcat已安装,跳过安装$color"
    else
        echo -e "$(info) 正在安装hashcat工具"
        $pkg_install hashcat $yes_tg
    fi
}

test_burpsuite() {
    if command -v burpsuite >/dev/null 2>&1; then
        echo -e "$green ◉ burpsuite已安装,跳过安装$color"
    else
        echo -e "$(info) 正在安装burpsuite工具"
        $pkg_install burpsuite $yes_tg
    fi
}

# docker安装脚本
test_docker() {
    if command -v docker >/dev/null 2>&1; then
        echo "docker已安装"
    else
        curl -fsSL https://get.docker.com | bash -s docker --mirror Aliyun
    fi
}

test_bastet() {
    echo "111"
}

#通用安装
test_install() {
    if [[ $language == "China" ]]; then
        if command -v $* >/dev/null 2>&1; then
            echo -e "$(info) $green $*已安装,跳过安装$color"
        else
            echo -e "$(info) 正在安装$*"
            if command -v eatmydata >/dev/null 2>&1; then
                eatmydata_setup=eatmydata
            fi
            $sudo_setup $eatmydata_setup $pkg_install $* $yes_tg
            install_error=$?
            if [ $install_error -ne 0 ]; then
                echo -e "$(info) $red $*安装失败。$color"
                echo -e "$(info) $red 错误代码$install_error $color"
                echo -e "$(info) 正在尝试更新软件包"
                $sudo_setup $pkg_update $yes_tg
                if [ $? -ne 0 ]; then
                    echo -e "$(info) $red 更新软件包失败$color"
                    esc
                else
                    echo -e "$(info) $green 更新软件包成功,正在尝试重新安装。$color"
                    $sudo_setup $eatmydata_setup $pkg_install $* $yes_tg
                fi
            else
                echo -e "$(info) $green $*安装成功。$color"
            fi
        fi
    else
        if command -v $* >/dev/null 2>&1; then
            echo -e "$(info) $green $* is installed, skipping installation $color"
        else
            echo -e "$(info) Installing $*"
            if command -v eatmydata >/dev/null 2>&1; then
                eatmydata_setup=eatmydata
            fi
            $sudo_setup $eatmydata_setup $pkg_install $* $yes_tg
            install_error=$?
            if [ $install_error -ne 0 ]; then
                echo -e "$(info) $red $* installation failed. $color"
                echo -e "$(info) $red Error code $install_error $color"
                echo -e "$(info) Attempting to update packages"
                $sudo_setup $pkg_update $yes_tg
                if [ $? -ne 0 ]; then
                    echo -e "$(info) $red Package update failed $color"
                    esc
                else
                    echo -e "$(info) $green Package update succeeded, attempting reinstall. $color"
                    $sudo_setup $eatmydata_setup $pkg_install $* $yes_tg
                fi
            else
                echo -e "$(info) $green $* installation succeeded. $color"
            fi
        fi
    fi
}

#通用卸载函数
test_remove() {
    if command -v $* >/dev/null 2>&1; then
        $sudo_setup $pkg_remove $* $yes_tg
        remove_error=$?
        if [[ $remove_error -ne 0 ]]; then
            echo -e "$(info) $red $* 软件包卸载失败,按回车键继续。 $color";read
        fi
    else
        echo -e "$(info) $green 不存在 $* 这个软件包，无需卸载$color"
    fi
}

#pip通用安装
pip_install() {
    test_install pip
    echo -e "$(info) 正在搜索本地pip库"
    if pip show "$*" > /dev/null 2>&1; then
       echo -e "$(info) $green ◉ $*已安装,跳过安装$color"
    else
        echo -e "$(info) 正在使用pip安装$*"
        pip install --break-system-packages $*
        if [ $? -ne 0 ]; then
            echo -e "$(info) $red pip安装$*失败$color"
        else
            echo -e "$(info) $green $*安装成功$color"
        fi
    fi
}

pipx_install() {
    eval PATH=$PATH:$HOME/.local/bin >/dev/null 2>&1
    
    test_install pipx
    pip install pipx
    echo -e "$(info) 正在搜索本地pip库"
    if pip show "$*" > /dev/null 2>&1; then
       echo -e "$(info) $green  $*已安装,跳过安装$color"
    else
        echo -e "$(info) 正在安装$*中"
        pipx install $* --force --break-system-packages
        if [ $? -ne 0 ]; then
            echo -e "$(info) $red $*安装失败$color"
        else
            echo -e "$(info) $green $*安装成功$color"
        fi
    fi
}

#通用克隆
git_clone() {
    github_speed_tool #地区检测
    if [[ -d $2 ]]; then
        $habit --title "确认操作" --yesno "当前仓库已克隆\n是否重新克隆一遍？\nThe repository has already been cloned.\nDo you want to clone it again?" 0 0
        if [ $? -ne 0 ]; then
            echo -e "$(info) 仓库重复，跳过克隆环节"
        else
            rm -rf $2
            git clone $github_speed$1 $2
            cw_test=$?
        fi
    else
        git clone $github_speed$1 $2
        cw_test=$?
    fi
    if [[ $cw_test -ne 0 ]]; then
        echo -e "$(info) $red 仓库克隆失败 Repository clone failed$color"
        echo -e "$(info) $red 错误代码 Error Code：$cw_test$color"
    else
        echo -e "$(info) $green 仓库克隆成功 Repository cloned successfully$color"
    fi
    esc
}

#通用下载
dow() {
    if command -v aria2c >/dev/null 2>&1; then
        aria2c -c -m 2 -s 16 -x 16 "$1" -d "$2"
        cw_test=$?
    elif command -v wget >/dev/null 2>&1; then
        wget -P "$2" "$1"
        cw_test=$?
    elif command -v curl >/dev/null 2>&1; then
        curl -o "$2/$(basename $1)" "$1"
        cw_test=$?
    else
        echo -e "$(info) $red 系统没有可用的下载器$color"
        echo -e "$(info) $red No downloaders are available on the system$color"
        esc
    fi
    if [[ $cw_test -eq 0 ]]; then
        echo -e "$(info) $green 下载成功 Download successful$color"
    else
        echo -e "$(info) $red 下载失败 Download failed$color"
        echo -e "$(info) $red 错误代码 Error Code：$cw_test$color"
        esc
    fi
}
tmux_keys() {
    if [[ $language == "China" ]]; then
        echo " Ctrl+b c: 创建一个新窗口"
        echo " Ctrl+b p: 切换到上一个窗口"
        echo " Ctrl+b n: 切换到下一个窗口"
        echo " Ctrl+b d: 退出终端连接"
    else
        echo " Ctrl b c: Create a new window, and the status bar will show information of multiple windows."
        echo " Ctrl b p: Switch to the previous window (according to the order on the status bar)."
        echo " Ctrl b n: Switch to the next window."
        echo " Ctrl b d: Exit terminal connection"
    fi
}
language_jc() {
    [[ -z $language ]] && language
}
language() {
    if [[ ! -v habit ]]; then
        temp=1
        if command -v whiptail >/dev/null 2>&1; then
            habit="whiptail"
        elif command -v dialog >/dev/null 2>&1; then
            habit="dialog"
        else
            test_whiptail
            habit="whiptail"
        fi
    fi
    if [[ -v 1 ]]; then
        if [[ $1 == "info" ]]; then
            echo -e "$(info) $language"
        elif [[ $1 == "list" ]]; then
            echo -e "简体中文"
            echo -e "繁体中文"
            echo -e "English"
            echo -e "Japanese"
        else
            config add language $1
        fi
        exit 0
    fi
    language_tx() {
        $habit --msgbox "语言设置完成\nlanguage Setup complete" 0 0
    }
    language_menu() {
        language_menu_xz=$($habit --clear --title "语言地区|language" \
            --menu "請選擇您的地區語言.\nPlease select your regional language.\n当前语言/present:$language" 0 0 10 \
            1 "自动检测   Auto" \
            2 "简体中文  China" \
            3 "英语   English" \
            0 "退出选择   quit" \
            2>&1 1>/dev/tty)
    }
    language_menu
    case $language_menu_xz in
        1)
            country
            language_tx
            ;;
        2)
            config add language China
            language_tx
            ;;
        3)
            config add language English
            language_tx
            ;;
        *)
            if [[ ! -v language ]]; then
                config add language English
            fi
            ;;
    esac
    if [[ $temp -eq 1 ]]; then
        unset habit
    fi
}

shell_setup() {
    if [[ $language == "China" ]]; then
       shell_setup_xz=$($habit --title "脚本设置" \
       --menu "脚本设置" 0 0 10 \
       1 "🌈 菜单个性化" \
       2 "🌈 颜色显示习惯" \
       3 "🚀 github加速" \
       4 "➕ 文件默认打开" \
       5 "📦 清理下载缓存" \
       6 "❌ 删除配置文件" \
       7 "📝 编辑配置文件" \
       8 "🌎 国家语言设置" \
       9 "🆕 版本更新检测" \
       10 "❌ 卸载管理工具" \
       0 "◀ 返回" \
       2>&1 1>/dev/tty)
    else
        shell_setup_xz=$($habit --title "Script Settings" \
        --menu "Script Settings" 0 0 10 \
        1 "🌈 Menu Personalization" \
        2 "🌈 Color Display Preferences" \
        3 "🚀 GitHub Acceleration" \
        4 "➕ Default Open" \
        5 "📦 Clear download cache" \
        6 "❌ Delete Configuration" \
        7 "📝 Configuration File" \
        8 "🌎 Language Settings" \
        9 "🆕 Version Update Check" \
        10 "❌ Uninstall Tool" \
        0 "◀ Back" \
        2>&1 1>/dev/tty)
    fi
}
#--------------------------------------------------------------------------------
# 最新版本检测
update_jc() {
    if [[ $update == "true" ]]; then
        new_tag_name=$(curl -sL 'https://api.gitcode.com/api/v5/repos/nasyt/nserver/releases/latest' | grep -m1 -o '"tag_name":"[^"]*"' | cut -d'"' -f4)
        if [[ $language == "China" ]]; then
            if [[ "nserver-$version" == "$new_tag_name" ]]; then
                update_jc="(没有新版本更新)"
            else
                update_jc="(有新版本更新$new_tag_name)"
            fi
        else
            if [[ "nserver-$version" == "$new_tag_name" ]]; then
                update_jc="(No new version update)"
            else
                update_jc="(new version update $new_tag_name)"
            fi
        fi
    fi
}

update_menu() {
    while true
    do
        update_menu_xz=$($habit --clear --title "自动检测更新" \
        --menu "当前设置:$(config load update)\n自动检测更新设置" 0 0 10 \
        true "开启" \
        false "关闭" \
        0 "◀返回" \
        2>&1 1>/dev/tty)
        case $update_menu_xz in
            true)
                config add update true
                ;;
            false)
                config add update false
                ;;
            *)
                break
                ;;
        esac
    done
}

# 更新工具
update() {
    bash -c "$(curl -L https://raw.gitcode.com/nasyt/nserver/raw/main/install.sh)"
    $habit --msgbox "nserver工具更新完成" 0 0
    exit 0
}
readme() {
    $habit --msgbox \
    "本工具由 NAS油条 制作\
    \n本工具免费提供大家使用\
    \n如有遇到什么问题请进群解答\n\
    \n本脚本的安装目录在 '~/.nserver' 目录\n\
    \n在查看tmux后台终端时的一些常用快捷键\n\
    \n$(tmux_keys)" 0 0
}

cache_admin() {
    if [[ -d $nasyt_dir/cache ]]; then
        $habit --yesno "是否删除以下文件\n总占用大小: $(du -sh $nasyt_dir/cache)\n$(br)\n$(du -sh $nasyt_dir/cache/*)\n$(br)" 0 0
        if [[ $? -eq 0 ]]; then
            rm -rfv $nasyt_dir/cache
            $habit --msgbox "删除完成" 0 0
        fi
    else
        $habit --msgbox "当前没有缓存" 0 0
    fi
}

must_tool() {
    update_jc
    if ! command -v jq >/dev/null 2>&1; then test_install jq;fi
    if ! command -v tmux >/dev/null 2>&1; then test_install tmux;fi
    if ! command -v micro >/dev/null 2>&1; then test_install micro;fi
    if ! command -v aria2c >/dev/null 2>&1; then
        if ! command -v wget >/dev/null 2>&1; then
            test_install aria2 || test_install aria2c
        fi
    fi
}
jvm_path() {
    [[ -n $PREFIX ]] && jvm_path=$PREFIX/lib/jvm || jvm_path=/usr/lib/jvm
}
java_dow_url() {
    myarch=$(uname -m)
    myuname=$(uname | tr 'A-Z' 'a-z')
    case "$myarch" in
        x86_64|amd64) arch="x64" ;;
        aarch64|arm64) arch="aarch64" ;;
        *) $habit --msgbox "不支持的架构: $myarch" 0 0;exit 1;;
    esac
    case $1 in
        25) echo "https://aka.ms/download-jdk/microsoft-jdk-25.0.3-${myuname}-${arch}.tar.gz" ;;
        21) echo "https://aka.ms/download-jdk/microsoft-jdk-21.0.11-${myuname}-${arch}.tar.gz" ;;
        17) echo "https://aka.ms/download-jdk/microsoft-jdk-17.0.19-${myuname}-${arch}.tar.gz" ;;
        11) echo "https://aka.ms/download-jdk/microsoft-jdk-11.0.31-${myuname}-${arch}.tar.gz" ;;
        8)  echo "https://mirrors.tuna.tsinghua.edu.cn/Adoptium/8/jdk/${arch}/${myuname}/OpenJDK8U-jdk_${arch}_${myuname}_hotspot_8u492b09.tar.gz" ;;
        *)  $habit --msgbox "不支持的版本: $ver (支持: 8, 11, 17, 21, 25)" 0 0; exit 1 ;;
    esac
    
    
}
jvm_get() {
    if [[ -z $PREFIX ]]; then
        jvm_dir=$(ls -1 /usr/lib/jvm/ | sed 's|/$||' | grep -E "^(jdk$1|jdk-$1)")
    else
        jvm_dir=$(ls -1 "$PREFIX/lib/jvm" | grep -E "^java-$1-")
    fi
    jvm_name=$(update-alternatives --quiet --list java 2>/dev/null | sed -E 's|.*/([^/]+)/bin/java|\1|' | grep -E "^(java-$1-|jdk$1|jdk-$1\.)")
}
java_index_sz() {
    $sudo_setup update-alternatives --set java $jvm_path/$1/bin/java
}
tar_xzvf() {
    dest="${2:-.}"
    mkdir -p "$dest" 
    total=$(tar -tf "$1" | wc -l)
    if [[ $total -eq 0 ]]; then
        tar -xf "$1" -C "$dest"
        return $?
    fi
    $sudo_setup tar -xvf "$1" -C "$dest" 2>/dev/null | awk -v total="$total" '
    {
        count++;
        printf "%d\n", count*100/total;
        fflush(stdout);
    }' | $habit --title "解压进度" --gauge "正在解压 $1" 8 65 0
}

java_dow_main() {
    if [[ -n $PREFIX ]]; then
        test_install openjdk-$java_dow_menu_xz
        [[ $? -ne 0 ]] && $habit --msgbox "安装openjdk-$java_dow_menu_xz失败" 0 0 || $habit --msgbox "openjdk-$java_dow_menu_xz安装完成" 0 0
    else
        java_url=$(java_dow_url $java_dow_menu_xz)
        java_url_name=$(basename $java_url)
        echo -e "$(info) 正在下载 $java_url_name,请耐心等待"
        dow $java_url $nasyt_dir/cache
        tar_xzvf $nasyt_dir/cache/$java_url_name "/usr/lib/jvm/";cw_test=$?
        jvm_get $java_dow_menu_xz
        $sudo_setup update-alternatives --install /usr/bin/java java /usr/lib/jvm/$jvm_dir/bin/java 100 >/dev/null 2>&1
        [[ $cw_test -ne 0 ]] && $habit --msgbox "$java_url_name\n解压失败" 0 0 || $habit --msgbox "openjdk-$java_dow_menu_xz安装完成" 0 0
    fi
}
mc_dow_input() {
    if [[ -z $mc_dow_json ]]; then
        mc_dow_json=$(curl -s https://api.mslmc.cn/v4/mirrors/ | jq -r)
    fi
}
dialog_menu_cl() {
    items=($(echo $mc_dow_json | jq -r ".data.$mc_dow_keys[]"))
    menu_args=()
    i=0
    for item in "${items[@]}"; do
        menu_args+=("$i" "$item")
        ((i++))
    done
}
dialog_menu_version() {
    mc_dow_version_url="https://api.mslmc.cn/v4/mirrors/$mc_dow_server"
    mc_dow_version_json=$(curl -s $mc_dow_version_url | jq -r)
    items2=($(echo $mc_dow_version_json | jq -r ".data.versions[]"))
    version_args=()
    i=0
    for item2 in "${items2[@]}"; do
        version_args+=("$i" "$item2")
        ((i++))
    done
}

mc_admin_version_list() {
    dir $nasyt_dir/mc dir_xz
    menu=()
    i=1
    for d in "${dir_xz[@]}"; do
        menu+=("$i" "$d")
        ((i++))
    done
}
mc_java_version_list() {
    dir $jvm_path java_xz
    jv_menu=()
    i=1
    for m in "${java_xz[@]}"; do
        jv_menu+=("$i" "$m")
        ((i++))
    done
}

tmux_id() {
    tmux list-sessions -F '#{session_id} #{session_name}' | awk -v n="$1" '$2==n{print $1;exit}'
}
mc_status() {
    if [[ $language == "China" ]]; then
        if tmux has-session -t $(tmux_id $(echo $mc_name | sed 's/\./_/g')) 2>/dev/null; then
            echo " ✅ 正在运行"
        else
            echo " 🚫 未运行"
        fi
    else
        if tmux has-session -t $(tmux_id $mc_name) 2>/dev/null; then
            echo " ✅ Running"
        else
            echo " 🚫 Not running"
        fi
    fi
}
java_version() {
    java_version=$(java --version | grep -E -o "openjdk\s\w.*")
}
mc_true() {
    true_menu_xz=$($habit --clear --title "状态设置" \
    --menu "当前设置:$2\n请选择" 0 0 10 \
    true "开启" \
    false "关闭" \
    0 "◀返回" \
    2>&1 1>/dev/tty)
    case $true_menu_xz in
        true)
            config add $3 true $1
            ;;
        false)
            config add $3 false $1
            ;;
        *)
            continue
            ;;
    esac
}

yml_true() {
    yml_menu_xz=$($habit --clear --title "状态设置" \
    --menu "当前设置: $(phira_yml $2)\n请选择" 0 0 10 \
    true "开启" \
    false "关闭" \
    0 "◀返回" \
    2>&1 1>/dev/tty)
    case $yml_menu_xz in
        true)
            yml add $1 $3 true
            ;;
        false)
            yml add $1 $3 false
            ;;
        *)
            continue
            ;;
    esac
}
input() {
    local var_name="$1"
    local prompt="$2"
    local default_val="$3"
    result=$($habit --clear --title "输入" \
    --inputbox "$prompt" 0 0 "$default_val" \
    2>&1 1>/dev/tty)
    cw_test=$?
    eval "$var_name='$result'"
}

phira_jc() {
    [[ -n $PREFIX ]] && myname=termux || myname=$(uname | tr 'A-Z' 'a-z')
    [[ -n $PREFIX ]] && myarch=arm64 || myarch=$(uname -m)
    case $myarch in
    x86_64) myarch="x64"
    esac
    phira_dow_name="phira-mp-$myname-$myarch.tar.gz"
    phira_name="phira-mp-$myname-$myarch"
    phira_config="$nasyt_dir/phira/config/server.yaml"
}
phira_dow() {
    echo -e "$(info) $blue 正在从 cnb.cool 下载最新服务端 $color"
    phira_jc
    dow https://cnb.cool/Pimeng233/gooophira-mp/-/releases/latest/download/$phira_dow_name $nasyt_dir/cache
    tar_xzvf $nasyt_dir/cache/$phira_dow_name $nasyt_dir/phira;cw_test=$?
    chmod +x $nasyt_dir/phira/$phira_name/*;cw_test=$?
    command -v phira_mp_server >/dev/null 2>&1 || config add PATH $nasyt_dir/phira/$phira_name:$PATH
    [[ $cw_test -ne 0 ]] && $habit --msgbox "服务端安装失败" 0 0 || $habit --msgbox "$phira_name 服务端安装完成" 0 0
}

phira_admin() {
    if command -v phira-mp-server >/dev/null 2>&1; then
        if [[ -e $nasyt_dir/phira/config/server.yaml ]]; then
            while true
            do
                phira_jc
                phira_admin_menu
                case $phira_admin_menu_xz in
                    1)
                        $open $phira_config
                        ;;
                    2)
                        phira_admin_name=$($habit --clear --title "服务器名称" \
                        --inputbox "请输入服务器名称" 0 0 \
                        2>&1 1>/dev/tty)
                        [[ $cw_test -ne 0 ]] || yml add $phira_config SERVER_NAME $phira_admin_name
                        ;;
                    3)
                        phira_admin_host=$($habit --clear --title "端口设置" \
                        --inputbox "请输入一个端口(0~65535)" 0 0 \
                        2>&1 1>/dev/tty)
                        [[ $cw_test -ne 0 ]] || yml add $phira_config HOST $phira_admin_host
                        ;;
                    4)
                        yml_true "$phira_config" "GUI" "GUI"
                        $habit --msgbox "网页管理地址为/gui" 0 0
                        ;;
                    5)
                        phira_admin_port=$($habit --clear --title "端口设置" \
                        --inputbox "网页管理地址为/gui\n请输入一个端口(0~65535)" 0 0 \
                        2>&1 1>/dev/tty)
                        [[ $cw_test -ne 0 ]] || yml add $phira_config HTTP_PORT $phira_admin_port
                        ;;
                    *)
                        break
                        ;;
                esac
            done
        else
            $habit --msgbox "请先启动一次服务端再来配置" 0 0
        fi
    else
        $habit --yesno "没有检测到phira服务端\n是否开始安装？" 0 0
        [[ $? -eq 0 ]] && phira_dow
    fi
}
phira_install_jc() {
    command -v phira-mp-server >/dev/null 2>&1 && echo "✅ 服务端已安装" || echo "❌ 未安装服务端"
}
phira_status() {
    if [[ $language == "China" ]]; then
        if tmux has-session -t phira_mp_server 2>/dev/null; then
            echo " ✅ 正在运行"
            [[ -v 1 ]] && echo " 🔗 连接端口: $(phira_yml PORT)"
        else
            echo " 🚫 未运行"
        fi
    else
        if tmux has-session -t phira_mp_server 2>/dev/null; then
            echo " ✅ Running"
        else
            echo " 🚫 Not running"
        fi
    fi
}
phira_start() {
    if command -v phira-mp-server >/dev/null 2>&1; then
        echo -e "$(info) 正在运行服务端"
        cd $nasyt_dir/phira
        tmux new -d -s phira_mp_server "phira-mp-server;$habit --msgbox "运行结束" 0 0"
        [[ -v 1 ]] && $habit --msgbox "phira-mp 服务端已运行\n连接端口:$(phira_yml PORT)" 0 0
    else
        $habit --yesno "没有检测到phira服务端\n是否开始安装？" 0 0
        [[ $? -eq 0 ]] && phira_dow
    fi
}

phira_stop() {
    tmux kill-session -t phira_mp_server || echo "当前已停止运行"
}
phira_link() {
    tmux attach -t phira_mp_server || echo -e "$red 连接失败请先启动 $color"
}
phira_rm() {
    $habit --title "确认操作" --yesno "你确定要删除服务端吗？" 0 0
    [[ $? -eq 0 ]] && rm -rfv $nasyt_dir/phira || continue
    $habit --msgbox "卸载服务端完成" 0 0
}
phira_yml() {
    phira_jc
    phira_config="$nasyt_dir/phira/config/server.yaml"
    yml load $phira_config $1
}

mindustry_dow_list() {
    mindustry_version_json=$(curl -sL https://api.github.com/repos/Anuken/Mindustry/releases | jq -r '.[].tag_name')
    mind_menu=()
    i=0
    for item3 in "${mindustry_version_json[@]}"; do
        mind_menu+=("$i" "$item3")
        ((i++))
    done
}

frp_path() {
    PATH=$nasyt_dir:$PATH
}

frp_dow() {
    if [[ -n $PREFIX ]]; then
        test_install frp
        cw_test=$?
    else
        OS=$(uname | tr '[:upper:]' '[:lower:]')
        ARCH=$(uname -m)
        case "$ARCH" in
            x86_64) ARCH="amd64" ;;
            aarch64) ARCH="arm64" ;;
            armv7l) ARCH="arm" ;;
            armv8l) ARCH="arm64" ;;
        esac
        [[ -n "$TERMUX_VERSION" ]] && OS="android"
        release_json=$(curl -sL "https://api.github.com/repos/fatedier/frp/releases/latest")
        if [[ -z "$release_json" ]]; then
            $habit --msgbox "❌ 获取github release信息失败" 0 0
            exit 1
        fi
        
        download_url=$(echo "$release_json" | jq -r --arg os "$OS" --arg arch "$ARCH" \
            '.assets[] | select(.name | endswith($os + "_" + $arch + ".tar.gz")) | .browser_download_url' | head -n1)
    
        if [[ -z "$download_url" ]]; then
            $habit --msgbox "❌ 未找到对应平台[$OS/$ARCH]的frp安装包" 0 0
            exit 1
        fi
        github_speed_tool
        frp_dow_url="$github_speed$download_url"
        frp_dow_name=$(basename $download_url)
        frp_name=$(echo $frp_dow_name | grep -o -E "frp_.*_\w*")
        dow $frp_dow_url $nasyt_dir/cache
        tar_xzvf $nasyt_dir/cache/$frp_dow_name $nasyt_dir/frp
        config add frp_name $frp_name
        chmod +x $nasyt_dir/frp/$frp_name/*
        cw_test=$?
        command -v frpc >/dev/null 2>&1 || config add PATH $nasyt_dir/frp/$frp_name:$PATH
    fi
    [[ $cw_test -ne 0 ]] && $habit --msgbox "frp可能安装失败" 0 0 || $habit --msgbox "frp安装完成" 0 0
}

frp_new_copy() {
    mkdir -p $nasyt_dir/frp_config
    input frp_new_name 请输入预设名字 name1;[[ $cw_test -ne 0 ]] && continue
    $habit --msgbox "请在接下来的文件中，粘贴配置文件\n\n记住\nCtrl+S保存 Ctrl+Q退出" 0 0
    micro $nasyt_dir/frp_config/$frp_new_name.toml
    $habit --yesno "确认配置文件\n$(cat $nasyt_dir/frp_config/$frp_new_name.toml)" 0 0;[[ $? -ne 0 ]] && continue
    $habit --msgbox "配置文件创建完成" 0 0
}

frp_arg_start() {
    frp_config_name=$(ls -1 $nasyt_dir/frp_config | grep $1)
    echo $frp_config_name
    if [[ -z $nasyt_dir/$frp_start_name ]]; then echo -e "$red 没有这个文件 $color";exit 1;fi
    echo -e "$green 正在启动 $frp_config_name 配置文件 $color"
    frp_start_name=$(echo $frp_config_name | sed "s/.toml//")
    tmux new -d -s $frp_start_name "frpc -c $nasyt_dir/frp_config/$frp_config_name;echo 运行结束;esc"
    echo -e "$green $frp_start_name 已后台运行服务 $color"
}

#--------------------------------------------------------------------------------
server_menu() {
    if [[ $language == "China" ]]; then
        server_menu_xz=$($habit --clear --title "服务端管理工具" \
        --backtitle "版本 version:$version    更新时间 update date:$time_date"\
        --menu "本脚本由 NAS油条 制作\nQQ群:610699712  $update_jc\n请使用方向键+回车键进行选择\n请选择你要启动的项目：" 0 0 10 \
        1 "🌈 我的世界   (Minecraft)" \
        2 "🕋 像素工厂   (mindustry)" \
        3 "🎵 phira音游  (phira server)" \
        4 "🚀 SFS联机    (sfs server)" \
        6 "📤 穿透工具   (Penetrating tool)" \
        7 "🆕 更新工具   (update Tool)" \
        8 "❗ 使用说明   (instructions)" \
        9 "🔨 工具设置   (setting)" \
        0 "🔴 退出工具   (exit)" \
        2>&1 1>/dev/tty)
    else
        server_menu_xz=$($habit --clear --title "Server Management Tool" \
        --backtitle "Version version:$version    Update date:$time_date"\
        --menu "This script is made by NAS Youtiao\nQQ Group: 610699712  $update_jc\nPlease use the arrow keys and Enter key to select\nPlease choose the project you want to start:" 0 0 10 \
        1 "🌈 Minecraft" \
        2 "🕋 Pixel Factory" \
        3 "🎵 phira Rhythm Game" \
        4 "🚀 SFS Online" \
        6 "📤 Penetration Tool" \
        7 "🆕 Update Tool" \
        8 "❗ Instructions" \
        9 "🔨 Tool Settings" \
        0 "🔴 Exit Tool" \
        2>&1 1>/dev/tty)
    fi
}
mc_server_menu() {
    if [[ $language == "China" ]]; then
        mc_server_xz=$($habit --clear --title "我的世界服务端管理" \
        --menu "请选择操作" 0 0 10 \
        1 "☕ java 下载" \
        2 "☕ java 管理" \
        3 "⏬ 服务端下载" \
        4 "🔨 服务端管理" \
        0 "◀返回" \
        2>&1 1>/dev/tty)
    else
        mc_server_xz=$($habit --clear --title "Minecraft Server Management" \
        --menu "Please choose" 0 0 10 \
        1 "☕ Java Download" \
        2 "⏬ Server Download" \
        3 "🔨 Server Management" \
        0 "◀ Back" \
        2>&1 1>/dev/tty)
    fi
}
java_dow_menu() {
    if [[ -n $PREFIX ]]; then
        java_dow_menu_xz=$($habit --clear --title "☕ Java下载" \
        --menu "检测到Termux终端\n推荐: java25\n请选择你要安装的Java" 0 0 10 \
        25 "Java25" \
        21 "Java21" \
        17 "Java17" \
        0 "◀返回" \
        2>&1 1>/dev/tty)
    else
        java_dow_menu_xz=$($habit --clear --title "☕ Java下载" \
        --menu "推荐: java25(适配大多数服务端)\n请选择你要安装的Java" 0 0 10 \
        25 "Java25" \
        21 "Java21" \
        17 "Java17" \
        11 "Java11" \
        8 "Java8" \
        0 "◀返回" \
        2>&1 1>/dev/tty)
    fi
}
java_admin_list() {
    java_admin_menu_list_xz=$($habit --clear --title "☕ java管理" \
    --menu "当前默认:$java_version\n请选择要管理的☕java" 0 0 10 \
    ${jv_menu[@]} \
    0 "◀返回" \
    2>&1 1>/dev/tty)
    cw_test=$?
    java_admin_list_xz="${java_xz[$java_admin_menu_list_xz-1]}"
}
java_admin_menu() {
    java_admin_menu_xz=$($habit --clear --title "java管理" \
    --menu "$java_admin_list_xz\n请选择操作" 0 0 10 \
    1 "设置为默认" \
    2 "删除当前java" \
    0 "◀返回" \
    2>&1 1>/dev/tty)
    cw_test=$?
}
mc_dow_menu() {
    if [[ $language == "China" ]]; then
        mc_dow_menu_xz=$($habit --clear --title "MC服务端下载" \
        --menu "API下载服务由 MSL开服器 提供(mslmc.cn)\n请选择一个分类" 0 0 0 \
        0 "✨ 插件服务端" \
        1 "🔨 插件与Forge" \
        2 "📜 插件与Fabric" \
        3 "🔨 模组Forge" \
        4 "📜 模组Fabric" \
        5 "🏢 原版服务端" \
        6 "⬛ 基岩版服务端" \
        7 "📶 代理服务端" \
        8 "➕ 更多服务端" \
        9 "◀返回" \
        2>&1 1>/dev/tty)
        cw_test=$?
    else
        mc_dow_menu_xz=$($habit --clear --title "MC Server Download" \
        --menu "API services provided by MSL (mslmc.cn)\nPlease select a category" 0 0 0 \
        0 "✨ Plugin Server" \
        1 "🔨 Plugins & Forge" \
        2 "📜 Plugins & Fabric" \
        3 "🔨 Mod Forge" \
        4 "📜 Mod Fabric" \
        5 "🏢 Vanilla Server" \
        6 "⬛ Bedrock Edition Server" \
        7 "📶 Proxy Server" \
        8 "➕ More Servers" \
        9 "◀ Back" \
        2>&1 1>/dev/tty)
        cw_test=$?
    fi
}
mc_dow_dialog_menu() {
    mc_dow_xz=$($habit --clear --title "服务端核心选择" \
    --menu "Please select a server core\n请选择一个服务端核心" 0 0 0 \
    ${menu_args[@]} \
    10 "◀返回" \
    2>&1 1>/dev/tty)
    cw_test=$?
}
mc_dow_version_menu() {
    mc_dow_version_xz=$($habit --clear --title "$mc_dow_server 服务端版本选择" \
    --menu "$mc_dow_server 服务端支持的版本如下\n请选择一个版本" 0 0 0 \
    ${version_args[@]} \
    78 "◀返回" \
    2>&1 1>/dev/tty)
    cw_test=$?
}
mc_admin_version_xz() {
    mc_admin_sz=$($habit --clear --title "服务端选择" \
    --menu "Please select the server to manage\n请选择要管理的服务器" 0 0 0 \
    ${menu[@]} \
    0 "◀返回" \
    2>&1 1>/dev/tty)
    cw_test=$?
    mc_name="${dir_xz[$mc_admin_sz-1]}"
    cd $nasyt_dir/mc/$mc_name
}
mc_mod_menu() {
    mc_mod_menu_xz=$($habit --clear --title "mod管理" \
    --menu "请选择要管理的mod" 0 0 10 \
    ${mod_menu[@]} \
    999 "◀返回" \
    2>&1 1>/dev/tty)
    cw_test=$?
    #mc_mod_name="${mc_mod_list[$mc_mod_menu_xz-1]}"
}
mindustry_dow_menu() {
    mind_version_sz=$($habit --clear --title "服务端选择" \
    --menu "Please select the server to manage\n请选择要下载的服务端" 0 0 0 \
    ${mind_menu[@]} \
    0 "◀返回" \
    2>&1 1>/dev/tty)
    cw_test=$?
    mind_name="${dir_xz[$mind_version_sz-1]}"
}
mc_mod_admin() {
    mc_mod_admin_xz=$($habit --clear --title "mod设置" \
    --menu "$(basename $mc_mod_name)\n请选择你要执行的操作" 0 0 10 \
    1 "禁用模组" \
    2 "删除模组" \
    0 "◀返回" \
    2>&1 1>/dev/tty)
    cw_test=$?
}
mc_admin_menu() {
    if [[ $language == "China" ]]; then
        mc_admin_menu_xz=$($habit --clear --title "服务器管理" \
        --menu "服务端: $mc_name\n运行状态:$(mc_status)\n端口: $(config load server-port 0 $nasyt_dir/mc/$mc_name/server.properties)\njava版本: $java_version\n请选择操作" 0 0 10 \
        1 "✅ 启动服务器" \
        2 "🚫 停止服务器" \
        3 "💻 进入终端管理" \
        4 "🔨 模组插件管理" \
        5 "📃 查看运行日志" \
        6 "📌 更多启动设置" \
        7 "📦 查看空间占用" \
        8 "📋 配置文件设置" \
        9 "❌ 删除服务器" \
        0 "◀返回" \
        2>&1 1>/dev/tty)
        cw_test=$?
    else
        mc_admin_menu_xz=$($habit --clear --title "Server Management" \
        --menu "Server: $mc_name\nStatus:$(mc_status)\nPort: $(config load server-port 0 $nasyt_dir/mc/$mc_name/server.properties)\nJava Version: $java_version\nPlease choose an action" 0 0 10 \
        1 "✅ Start Server" \
        2 "🚫 Stop Server" \
        3 "💻 Enter Terminal Management" \
        4 "🔨 Module Plugin Management" \
        5 "📃 View Running Log" \
        6 "📌 More Startup Settings" \
        7 "📦 View space usage" \
        8 "📋 Configuration Settings" \
        9 "❌ Delete Server" \
        0 "◀Back" \
        2>&1 1>/dev/tty)
        cw_test=$?
    fi
}
mc_pluginmod_menu() {
    mc_pluginmod_menu_xz=$($habit --clear --title "插件模组管理" \
    --menu "请选择操作" 0 0 10 \
    1 "插件管理" \
    2 "模组管理" \
    0 "◀返回" \
    2>&1 1>/dev/tty)
    cw_test=$?
}
mc_start_menu() {
    mc_start_menu_xz=$($habit --clear --title "更多启动设置" \
    --menu "请选择你的操作" 0 0 10 \
    1 "自定义jar文件启动" \
    0 "◀返回" \
    2>&1 1>/dev/tty)
}
mc_config_menu() {
    mc_config_menu_xz=$($habit --clear --title "配置设置" \
    --menu "$mc_name 服务端的配置文件\n请选择你要配置的选项" 0 0 10 \
    1 "显示名称:     $motd" \
    2 "连接端口:     $server_port" \
    3 "正版验证:     $online_mode" \
    4 "人数上限:     $max_players" \
    5 "允许飞行:     $allow_flight" \
    6 "游戏模式:     $gamemode" \
    7 "游戏难度:     $difficulty" \
    8 "允许下界:     $allow_nether" \
    9 "生成结构:     $generate_structures" \
    10 "生成动物:     $spawn_animals" \
    11 "生成怪物:     $spawn_monsters" \
    12 "生成 NPC:     $spawn_npcs" \
    13 "视野距离:     $view_distance" \
    14 "成就显示:     $announce_player_achievements" \
    15 "最大建造高度: $max_build_height" \
    16 "最大世界大小: $max_world_size" \
    0 "◀返回" \
    2>&1 1>/dev/tty)
}








mindustry_menu() {
    mindustry_menu_xz=$($habit --clear --title "mindustry管理" \
    --menu "请选择你的操作" 0 0 10 \
    1 "java下载" \
    2 "java管理" \
    3 "服务端下载" \
    4 "服务端管理" \
    0 "◀返回" \
    2>&1 1>/dev/tty)
}

phira_menu() {
    phira_menu_xz=$($habit --clear --title "phira服务端管理" \
    --menu "安装状态: $(phira_install_jc)\n运行状态:$(phira_status)\n请选择操作" 0 0 10 \
    1 "下载服务端" \
    2 "管理服务端" \
    3 "启动服务端" \
    4 "连接后台终端" \
    5 "停止运行" \
    6 "卸载服务端" \
    0 "◀返回" \
    2>&1 1>/dev/tty)
}

phira_admin_menu() {
    phira_admin_menu_xz=$($habit --clear --title "配置文件管理" \
    --menu "请选择你的操作" 0 0 10 \
    1 "手动编辑配置文件" \
    2 "服务器名称:  $(phira_yml SERVER_NAME)" \
    3 "服务器端口:  $(phira_yml PORT)" \
    4 "网页管理:    $(phira_yml GUI)" \
    5 "网页端口:    $(phira_yml HTTP_PORT)" \
    0 "◀返回" \
    2>&1 1>/dev/tty)
}

frp_menu() {
    frp_menu_xz=$($habit --clear --title "穿透工具" \
    --menu "请选择一款工具" 0 0 10 \
    1 "frp" \
    2 "cpolar" \
    0 "◀返回" \
    2>&1 1>/dev/tty)
}

frp_use_menu() {
    frp_use_menu_xz=$($habit --clear --title "frp管理工具" \
    --menu "frp内网穿透管理工具\n请选择你的操作" 0 0 10 \
    1 "安装更新frp" \
    2 "添加配置文件" \
    3 "管理配置文件" \
    4 "启动frp穿透" \
    5 "后台管理工具" \
    0 "◀返回" \
    2>&1 1>/dev/tty)
}

frp_new_menu() {
    frp_new_menu_xz=$($habit --clear --title "添加配置文件" \
    --menu "请选择你的操作" 0 0 10 \
    1 "自定义配置创建" \
    2 "粘贴导入创建" \
    0 "◀返回" \
    2>&1 1>/dev/tty)
}

frp_admin_menu() {
    frp_admin_menu_xz=$($habit --clear --title "配置文件管理" \
    --menu "$(basename $frp_admin_xz)配置文件管理\n请选择操作" 0 0 10 \
    1 "📝 编辑文件" \
    2 "➕ 重命名" \
    3 "🆕 复制文件" \
    4 "❌ 删除文件" \
    0 "◀返回" \
    2>&1 1>/dev/tty)
}

#--------------------------------------------------------------------------------
#--------------------------------------------------------------------------------
mc_server() {
    while true
    do
        mc_server_menu
        case $mc_server_xz in
            1) java_dow;;
            2) java_admin;;
            3) mc_dow;;
            4) mc_admin;;
            *) break;;
        esac
    done
}

java_dow() {
    while true
    do
        java_dow_menu;[[ $java_dow_menu_xz -eq 0 ]] && break
        java_dow_main
    done
}
mc_dow() {
    while true
    do
        mc_dow_input
        mc_dow_menu;[[ $cw_test -ne 0 ]] && break;[[ $mc_dow_menu_xz -eq 9 ]] && break
        mc_dow_keys=$(echo $mc_dow_json | jq ".data | keys_unsorted[$mc_dow_menu_xz]")
        
        dialog_menu_cl
        mc_dow_dialog_menu;[[ $cw_test -ne 0 ]] && continue;[[ $mc_dow_xz -eq 10 ]] && continue
        mc_dow_server=$(echo $mc_dow_json | jq -r ".data.$mc_dow_keys[$mc_dow_xz]")
        
        dialog_menu_version
        mc_dow_version_menu;[[ $cw_test -ne 0 ]] && continue;[[ $mc_dow_version_xz -eq 78 ]] && continue
        mc_dow_version=$(echo $mc_dow_version_json | jq -r ".data.versions[$mc_dow_version_xz]")
        
        mc_dow_url=$(curl -s "https://api.mslmc.cn/v4/download/server/$mc_dow_server/$mc_dow_version" | jq -r .data.url)
        mc_name=$mc_dow_server-$mc_dow_version
        mkdir -p $nasyt_dir/mc/$mc_name
        dow $mc_dow_url $nasyt_dir/mc/$mc_name
        mv $nasyt_dir/mc/$mc_name/*.jar $nasyt_dir/mc/$mc_name/server.jar >/dev/null 2>&1
        esc;[[ $cw_test -ne 0 ]] && $habit --msgbox "下载失败，错误代码: $cw_test" 0 0 ||$habit --msgbox "$mc_name\n服务端下载成功" 0 0
        break
    done
}
java_admin() {
    while true
    do
        java_version
        mc_java_version_list
        java_admin_list;[[ $cw_test -ne 0 ]] && break;[[ $java_admin_menu_list_xz -eq 0 ]] && break
        while true
        do
            java_admin_menu
            case $java_admin_menu_xz in
                1)
                    java_index_sz $java_admin_list_xz
                    $habit --msgbox "默认java设置完成" 0 0
                    break
                    ;;
                2)
                    $habit --title "确认操作" --yesno "你确定要卸载 $java_admin_list_xz 吗？" 0 0
                    if [[ $? -ne 0 ]]; then
                        break
                    fi
                    if [[ -n $PREFIX ]]; then
                        $pkg_remove -y openjdk-$(echo $java_admin_list_xz | grep -o -E "[0-9]+")
                    else
                        $sudo_setup rm -rf $jvm_path/$java_admin_list_xz
                    fi
                    $habit --msgbox "卸载 $java_admin_list_xz 完成" 0 0
                    break
                    ;;
                *)
                    break;;
            esac
        done
    done
}
mc_admin() {
    while true
    do
        ls -1d "$nasyt_dir/mc"/*/ 2>/dev/null;
        if [[ $? -ne 0 ]];then
            $habit --msgbox "请先下载服务端\nPlease download the server first" 0 0;break
        fi
        mc_admin_version_list
        mc_admin_version_xz;[[ $cw_test -ne 0 ]] && break;[[ $mc_admin_sz -eq 0 ]] && break
        while true
        do
            java_version
            config_file=$nasyt_dir/mc/$mc_name/server.properties
            mc_admin_menu
            case $mc_admin_menu_xz in
                1)
                    config add eula true $nasyt_dir/mc/$mc_name/eula.txt
                    tmux new -d -s $(echo $mc_name | sed 's/\./_/g') "java -jar $nasyt_dir/mc/$mc_name/server.jar;${esc};$habit --msgbox "运行结束" 0 0"
                    $habit --msgbox "服务器已后台启动\n请前往终端管理查看" 0 0
                    ;;
                2)
                    tmux kill-session -t $(tmux_id $(echo $mc_name | sed 's/\./_/g'))
                    $habit --msgbox "停止 $mc_name 服务器成功" 0 0
                    ;;
                3)
                    tmux attach -t $(tmux_id $(echo $mc_name | sed 's/\./_/g'))
                    $habit --msgbox "已断开 $mc_name 服务器终端" 0 0
                    ;;
                4)
                    mc_pluginmod
                    ;;
                5)
                    file_xz $nasyt_dir/mc/$mc_name/logs mc_start_log
                    [[ $cw_test -ne 0 ]] && continue
                    nano $mc_start_log
                    ;;
                6)
                    mc_start
                    ;;
                7)
                    test_install ncdu
                    ncdu $nasyt_dir/mc/$mc_name
                    ;;
                8)
                    mc_config
                    ;;
                9)
                    $habit --title "确认操作" --yesno "你确定要删除 $mc_name 服务器吗？" 0 0
                    if [[ $? -eq 0 ]]; then
                        rm -rfv $nasyt_dir/mc/$mc_name
                        $habit --msgbox "删除 $mc_name 服务器完成" 0 0
                        break
                    fi
                    ;;
                *)
                    break
                    ;;
            esac
        done
    done
}
mc_pluginmod() {
    while true
    do
        mc_pluginmod_menu
        case $mc_pluginmod_menu_xz in
            1)
                #mc_plugin
                $habit --msgbox "开发中" 0 0
                ;;
            2)
                mc_mod
                ;;
            *)
                break
                ;;
        esac
    done
}
mc_mod() {
    while true
    do
        file_xz $nasyt_dir/mc/$mc_name/mods mc_mod_name
        [[ $cw_test -ne 0 ]] && continue
        mc_mod_admin
        case $mc_mod_admin_xz in
            1)
                mv $mc_mod_name $mc_mod_name.disabled
                $habit --msgbox "模组禁用完成" 0 0
                ;;
            2)
                rm $mc_mod_name
                $habit --msgbox "模组删除完成" 0 0
                ;;
            *)
                break
                ;;
        esac
    done
}
mc_start() {
    while true
    do
        mc_start_menu
        case $mc_start_menu_xz in
            1)
                file_xz $nasyt_dir/mc/$mc_name mc_start_file jar
                [[ $cw_test -ne 0 ]] && continue
                config add eula true $nasyt_dir/mc/$mc_name/eula.txt
                tmux new -d -s $mc_name "java -jar $mc_start_file;esc;$habit --msgbox "运行结束" 0 0"
                $habit --msgbox "服务器已后台启动\n请前往终端管理查看" 0 0
                ;;
            *)
                break;;
        esac
    done
}
mc_config() {
    while true
    do
        #mc_config_all
        eval "$(cat $config_file | sed 's/-/_/g' | sed -E 's/^(.*)=(.*)$/\1="\2"/')"
        mc_config_all=$(cat $config_file | sed 's/./_/g' | sed 's/-/_/g' | sed 's/#.*//' | grep -o "^[^=]*")
        mc_config_menu
        case $mc_config_menu_xz in
            1)
                motd=$($habit --clear --title "名称设置" \
                --inputbox "❗不支持中文输入\n请输入服务器名称" 0 0 $motd\
                2>&1 1>/dev/tty);[[ $? -ne 0 ]] && continue
                config add motd "$motd" $config_file
                ;;
            2)
                server_port=$($habit --clear --title "端口设置" \
                --inputbox "请输入服务器端口" 0 0 $server_port\
                2>&1 1>/dev/tty);[[ $? -ne 0 ]] && continue
                config add "server-port" "$server_port" $config_file
                ;;
            3)
                mc_true "$config_file" "online_mode" "online-mode"
                ;;
            4)
                max_players=$($habit --clear --title "人数设置" \
                --inputbox "请输入最大人数" 0 0 $max_players\
                2>&1 1>/dev/tty);[[ $? -ne 0 ]] && continue
                config add max-players "$max_players" $config_file
                ;;
            5)
                mc_true "$config_file" "allow_flight" "allow-flight"
                ;;
            6)
                gamemode=$($habit --clear --title "游戏模式" \
                --inputbox "(0) - 生存模式creative \n(1) - 创造模式\nadventure (2) - 冒险模式\nspectator - 旁观模式 \n请输入数字" 0 0 $gamemode\
                2>&1 1>/dev/tty);[[ $? -ne 0 ]] && continue
                config add gamemode "$gamemode" $config_file
                ;; 
            7)
                difficulty=$($habit --clear --title "难度设置" \
                --inputbox "0 和平\n1 简单\n2 普通\n3 困难\n请输入数字" 0 0 $difficulty\
                2>&1 1>/dev/tty);[[ $? -ne 0 ]] && continue
                config add difficulty "$difficulty" $config_file
                ;;
            8)
                mc_true "$config_file" "allow_nether" "allow-nether"
                ;;
            9)
                mc_true "$config_file" "generate_structures" "generate-structures"
                ;;
            10)
                mc_true "$config_file" "spawn_animals" "spawn-animals"
                ;;
            11)
                mc_true "$config_file" "spawn_monsters" "spawn-monsters"
                ;;
            12)
                mc_true "$config_file" "spawn_npcs" "spawn-npcs"
                ;;
            13)
                view_distance=$($habit --clear --title "视野距离" \
                --inputbox "(影响服务器性能)\n请输入视野距离" 0 0 $view_distance\
                2>&1 1>/dev/tty)
                config add view-distance "$view_distance" $config_file
                ;;
            14)
                mc_true "$config_file" "announce_player_achievements" "announce-player-achievements"
                ;;
            14)
                max_build_height=$($habit --clear --title "最大建造高度" \
                --inputbox "请输入最大建造高度" 0 0 $max_build_height\
                2>&1 1>/dev/tty)
                config add max-build-height "$max_build_height" $config_file
                ;;
            15)
                max_world_size=$($habit --clear --title "最大世界大小" \
                --inputbox "请输入最大世界大小" 0 0 $max_world_size\
                2>&1 1>/dev/tty)
                config add max-world-size "$max_world_size" $config_file
                ;;
            *) break;;
        esac
    done
}

mindustry_server() {
    while true
    do
        mindustry_menu
        case $mindustry_menu_xz in
            1) java_dow;;
            2) java_admin;;
            3) $habit --msgbox "开发中" 0 0;mindustry_dow;;
            4) $habit --msgbox "开发中" 0 0;mindustry_admin;;
            *) break;;
        esac
    done
}
mindustry_dow() {
    while true
    do
        mindustry_dow_list
        mindustry_dow_menu
        
        
        $mindustry_dow_menu_xz
        
    done
}

phira_server() {
    while true
    do
        phira_menu
        case $phira_menu_xz in
            1) phira_dow;;
            2) phira_admin;;
            3) phira_start;;
            4) phira_link;;
            5) phira_stop;$habit --msgbox "phira-mp 服务端已停止运行" 0 0;;
            6) phira_rm;;
            *) break;;
        esac
    done
}

frp_tool() {
    while true
    do
        frp_path
        frp_menu
        case $frp_menu_xz in
            1) frp_use;;
            2) cpolar_tool;;
            *) break;;
        esac
    done
}

frp_use() {
    while true
    do
        frp_use_menu
        case $frp_use_menu_xz in
            1)
                frp_dow
                ;;
            2)
                frp_new
                ;;
            3)
                frp_admin
                ;;
            4)
                frp_start
                ;;
            5)
                tmux_tool
                ;;
            0)
                break
                ;;
            *)
                $habit --msgbox "开发中" 0 0
                ;;
        esac
    done
}

frp_new() {
    while true
    do
        frp_new_menu
        case $frp_new_menu_xz in
            1)
                frp_new_txt
                ;;
            2)
                frp_new_copy
                ;;
            *)
                break
                ;;
        esac
    done
}

frp_new_txt() {
    while true
    do
        mkdir -p $nasyt_dir/frp_config
        input frp_new_ip 请输入frp服务端的IP或域名 127.0.0.1;[[ $cw_test -ne 0 ]] && break
        input frp_new_port 请输入frp服务端的端口 7000;[[ $cw_test -ne 0 ]] && break
        input frp_new_token 请输入frp的验证token 114514;[[ $cw_test -ne 0 ]] && break
        input frp_new_protocol "tcp\nudp\nhttp\nhttps\n请输入要穿透的协议\n" tcp;[[ $cw_test -ne 0 ]] && break
        input frp_new_local_port 请输入要公开的本地端口 25565;[[ $cw_test -ne 0 ]] && break
        case $frp_new_protocol in
        tcp|udp) input frp_new_far_port 请输入远程访问的端口 7891;[[ $cw_test -ne 0 ]] && break || frp_new_far="remotePort = $frp_new_far_port # 远程端口";;
        http|https) input frp_new_far_http 请输入远程访问的域名 xxx.91.com;[[ $cw_test -ne 0 ]] && break || frp_new_far="customDomains = [ \"$frp_new_far_http\" ] # 远程域名";;esac
        input frp_new_name 请输入预设名字 name1;[[ $cw_test -ne 0 ]] && break
        frp_new_toml="serverAddr = \"$frp_new_ip\" # 服务端ip或域名
serverPort = $frp_new_port # 服务端端口

[auth]
token = \"$frp_new_token\" # 验证token

[[proxies]]
name = \"$frp_new_name\" # 配置名称
type = \"$frp_new_protocol\" # 连接协议
localIP = \"0.0.0.0\" # 本地IP
localPort = $frp_new_local_port # 本地端口
${frp_new_far}
"
        $habit --yesno "请确认配置文件\n$frp_new_toml" 0 0
        [[ $? -ne 0 ]] && break
        echo "$frp_new_toml" > $nasyt_dir/frp_config/$frp_new_name.toml
        $habit --msgbox "配置文件创建完成" 0 0
        break
    done
}

frp_admin() {
    while true
    do
        file_xz $nasyt_dir/frp_config frp_admin_xz toml;[[ $cw_test -ne 0 ]] && break
        frp_admin_menu
        case $frp_admin_menu_xz in
            1)
                micro $frp_admin_xz
                ;;
            2)
                new_name=$($habit --clear --title "请输入新名字" --inputbox "请输入新名字" 0 0 $(basename $frp_menu_xz) 2>&1 1>/dev/tty)
                mv $frp_admin_xz $nasyt_dir/frp_config/$new_name
                ;;
            3)
                new_name=$($habit --clear --title "请输入名字" --inputbox "请输入拷贝的新名字" 0 0 $(basename $frp_menu_xz) 2>&1 1>/dev/tty)
                cp $frp_admin_xz "$new_name"
                ;;
            4)
                rm $frp_admin_xz;[[ $? -ne 0 ]] && $habit --msgbox "删除失败" 0 0 || $habit --msgbox "删除完成" 0 0
                ;;
            *)
                break
                ;;
        esac
    done
}

frp_start() {
    file_xz $nasyt_dir/frp_config frp_start_xz toml
    frp_config_name=$(basename $frp_start_xz);[[ $? -ne 0 ]] && return 1
    if [[ -z $frp_start_xz ]]; then echo -e "$red 没有这个文件 $color";exit 1;fi
    echo -e "$green 正在启动 $frp_start_name 配置文件 $color"
    frp_start_name=$(echo $frp_config_name | sed "s/.toml//")
    tmux new -d -s $frp_start_name "frpc -c $frp_start_xz;echo 运行结束;esc"
    $habit --msgbox "$frp_start_name 已后台运行服务\n请前往后台管理工具查看" 0 0
}

frp_stop() {
    if [[ -z $1 ]];then
        echo -e "$red 还需要一个参数 $color"
    else
        tmux kill-session -t $(ls -1 $nasyt_dir/frp_config | sed "s/.toml//" | grep $1)
        echo "当前已停止运行"
    fi
}

frp_link() {
    if [[ -z $1 ]];then
        echo -e "$red 还需要一个参数 $color"
    else
        tmux attach -t $(ls -1 $nasyt_dir/frp_config | sed "s/.toml//" | grep $1)
        echo "已断开连接"
    fi
}

frp_status() {
    frp_status_config=$(ls -1 $nasyt_dir/frp_config | sed "s/.toml//")
    frp_status_list=$(tmux ls 2>/dev/null | grep -o "\w*: " | sed s/:.//)
    for conf_name in $frp_status_config; do
        local is_run=0
        for sess_name in $frp_status_list; do
            if [[ "$conf_name" == "$sess_name" ]]; then
                is_run=1
                break
            fi
        done
        if [[ $is_run -eq 1 ]]; then
            echo "✅ $conf_name 运行中"
        else
            echo "❌ $conf_name 未运行"
        fi
    done
}

config_setup() {
    while true
    do
        shell_setup
        case $shell_setup_xz in
            1) habit_xz;;
            2) color_menu;;
            3) github_speed_skip=1;github_speed_tool;;
            4) default_open;;
            5) cache_admin;;
            6) rm $nasyt_dir/config.txt;;
            7) $open $nasyt_dir/config.txt;;
            8) language;;
            9) update_menu;;
            10) rm $PREFIX/bin/nserver >/dev/null 2>&1;;
            *) break;;
        esac
    done
}

# 主执行函数
main() {
    must_tool           # 必备工具
    while true
    do
        server_menu
        case $server_menu_xz in
            1)
                mc_server;;
            2)
                mindustry_server;;
            3)
                phira_server;;
            4)
                $habit --msgbox "开发中" 0 0;sfs_server;;
            6)
                frp_tool;;
            7)
                update;;
            8)
                readme;;
            9)
                config_setup;;
            *)
                break;;
        esac
    done
    #clear
}



#--------------------------------------------------------------------------------


# 主程序
check() {
    color_variable      # 颜色变量
    check_pkg_install   # 系统检测
    config;yml          # 配置文件
    usevar              # 常用变量
    default_habit       # 默认菜单
    language_jc         # 语言检测
    eula                # 用户协议
}

check
#函数加载
jvm_path

if [[ -v 1 ]]; then
    case $1 in
        j|-j|jc|--jc)
            update_menu
            exit 0
            ;;
        c|-c|color|--color)
            if [[ -v 2 ]]; then
                config add color_var $2
            else
                color_menu
            fi
            exit 0
            ;;
        con|config|-config|--config)
            config $2
            exit
            ;;
        f|-f|--frp|frp)
            if [[ -v 2 ]]; then
                case $2 in
                    n|new) frp_new;;
                    a|admin) frp_admin;;
                    ls|l|list) frp_status;;
                    link) frp_link $3;;
                    st|status) frp_status;;
                    s|stop) frp_stop $3;;
                    *) frp_arg_start $2;;
                esac
            else    
                frp_tool
            fi
            exit
            ;;
        t|-t|--tmux)
            tmux_tool
            exit
            ;;
        u|-u|--update)
            update
            exit
            ;;
        v|-v|--version|version)
            shell_info
            exit 0
            ;;
        c|config)
            echo "脚本配置文件"
            config $2
            exit
            ;;
        dir|d)
            dir $2 $3
            exit
            ;;
        de|debug|--debug)
            if [[ -v 2 ]]; then
                "$2"
                exit
            else
                rm $nasyt_dir/tool.log >/dev/null 2>&1
                set -x
                > $nasyt_dir/tool.log
                exec > >(tee -a "$nasyt_dir/tool.log") 2>&1
            fi
            ;;
        l|language|-l)
            language
            exit
            ;;
        p|-p|--phira|phira)
            if [[ -v 2 ]]; then
                case $2 in
                    a|admin) phira_admin ;;
                    st|status) phira_status tty;;
                    s|start) phira_start ;;
                    l|link) phira_link ;;
                    t|stop) phira_stop ;;
                    *) echo -e "$red 无效的输入 $color"
                esac
            else
                phira_server
            fi
            exit 0
            ;;
        m|mc|-m|--mc)
            if [[ -v 2 ]]; then
                case $2 in
                    j|java|j|-j) java_admin;;
                    s|server) mc_admin;;
                    l|list) ls -1 $nasyt_dir/mc;;
                    *) echo "无效的输入"
                esac
            else
                mc_server
            fi
            exit
            ;;
        n|-n|ncdu|--ncdu)
            test_install ncdu
            ncdu $nasyt_dir
            exit
            ;;
        y|yml|-y)
            yml $2
            exit 0
            ;;
        h|help|-h|--help)
            if [[ $language == "China" ]]; then
                echo -e
                echo -e "${grey}用法:$color"
                echo -e "  ${blue}nserver$color ${pink}[参数]$color ${green}(参数)$color"
                echo -e "${grey}[参数]:$color"
                echo -e "$pink  c, --color          颜色输出设置$color"
                echo -e "$pink  f, --frp            内网穿透管理$color"
                echo -e "$green         (new)           新建配置文件 $color"
                echo -e "$green         (admin)         管理配置文件 $color"
                echo -e "$green         (list)          输出配置列表$color"
                echo -e "$green         (stop)          停止后台服务 $color"
                echo -e "$green         (link)          连接后台终端 $color"
                echo -e "$green         <文件名>        启动配置文件$color"
                echo -e "$pink  h, --help           输出帮助菜单$color"
                echo -e "$pink  l, --language       工具语言设置$color"
                echo -e "$pink  m, --mc             MC服务端管理$color"
                echo -e "$green         (java)          java环境管理$color"
                echo -e "$green         (server)        mc服务端管理$color"
                echo -e "$green         (list)          已安装的列表$color"
                echo -e "$pink  n, --ncdu           查看空间占用$color"
                echo -e "$pink  p, --phira          phira服务端 $color"
                echo -e "$green         (admin)         管理配置文件$color"
                echo -e "$green         (status)        查看运行状态$color"
                echo -e "$green         (start)         后台启动服务$color"
                echo -e "$green         (stop)          停止后台服务$color"
                echo -e "$green         (link)          连接后台终端$color"
                echo -e "$pink  t, --tmux           后台管理工具$color"
                echo -e "$pink  u, --update         更新管理工具$color"
                echo -e "$pink  v, --version        输出版本信息$color"
                echo -e
                echo -e "${grey}其他参数:$color"
                echo -e "$pink  config              配置文件函数 $color"
                echo -e "$green         (list)          列出配置文件$color"
                echo -e "$pink  debug               工具调试模式$color"
                echo -e "$pink  jc                  更新检测设置$color"
                echo -e "$green         (函数)          函数调试$color"
                echo -e ""
            else
                echo -e
                echo -e "${grey}Usage:$color"
                echo -e "  ${blue}nserver$color ${pink}[Parameter]$color ${green}(Parameter)$color"
                echo -e "${grey}[Parameter]:$color"
                echo -e "$pink  c, --color          Color output setting$color"
                echo -e "$pink  f, --frp            Intranet penetration management$color"
                echo -e "$green         (new)           Create a new configuration file $color"
                echo -e "$green         (admin)         Manage configuration files $color"
                echo -e "$green         (list)          Output configuration list$color"
                echo -e "$green         (filename)      Start configuration file$color"
                echo -e "$pink  h, --help           Output help menu$color"
                echo -e "$pink  l, --language       Tool language settings$color"
                echo -e "$pink  m, --mc             MC server management$color"
                echo -e "$green         (java)          Java environment management$color"
                echo -e "$green         (server)        MC server management$color"
                echo -e "$green         (list)          Installed list$color"
                echo -e "$pink  n, --ncdu           Check disk usage$color"
                echo -e "$pink  t, --tmux           Background management tool$color"
                echo -e "$pink  u, --update         Update management tool$color"
                echo -e "$pink  v, --version        Output version information$color"
                echo -e
                echo -e "${grey}Other parameters:$color"
                echo -e "$pink  debug               Tool debug mode$color"
                echo -e "$pink  jc                  Update Check Settings$color"
                echo -e "$green         (function)         Run function$color"
            fi
            exit 0
            ;;
        *)
            $habit --msgbox "无效的输入" 0 0
            ;;
    esac
fi

main