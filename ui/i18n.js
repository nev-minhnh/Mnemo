// Vietnamese / English. The interface is written in Vietnamese; when English is chosen,
// every interface text (text nodes, titles, placeholders) is swapped through this
// dictionary as it appears. Anything inside [data-noi18n] (messages, names, AI answers)
// is never touched.
'use strict';

window.LANG = 'vi';

const I18N = {
  '(chuột phải: cài đặt)': '(right-click: settings)',
  '(chưa có key)': '(no key yet)',
  '(hoặc mở một chat → ⋮ → Export chat history). Chọn định dạng': '(or open a chat → ⋮ → Export chat history). Choose',
  ', thích văn phong ngắn gọn, thân thiện.': ', likes a short, friendly style.',
  ', định dạng': ', format',
  '. Chọn tài khoản Facebook hoặc Instagram, chọn mục': '. Choose the Facebook or Instagram account, select',
  '. Khi Meta gửi xong, tải tệp .zip về (có thể nhiều phần, nhập lần lượt từng tệp).': '. When Meta is done, download the .zip (it may come in parts; import them one by one).',
  '0 = giữ mãi. Tệp bạn tự tải về không bao giờ bị tự xóa.': '0 = keep forever. Files you download yourself are never deleted automatically.',
  '0 = không giới hạn (tối đa vẫn là 512 MB).': '0 = no limit (512 MB is still the maximum).',
  'AI không đưa ra gợi ý nào': 'The AI gave no suggestions',
  'AI đang xử lý…': 'AI is working…',
  'AI đang đọc các hội thoại…': 'AI is reading the conversations…',
  'Ai có khóa này và tệp mnemo.db đều đọc được toàn bộ tin nhắn.': 'Anyone with this key and the mnemo.db file can read every message.',
  'Bản nháp (sửa trực tiếp được)': 'Draft (you can edit it)',
  'Bấm × thì thu về khay': 'Closing (×) minimizes to the tray',
  'Bấm để mở hội thoại': 'Click to open the conversation',
  'Bật': 'On',
  'Bật (mở qua Mnemo)': 'On (opened through Mnemo)',
  'Bật (tốn token)': 'On (uses tokens)',
  'Bật suy nghĩ sâu (thinking) – chậm hơn, kỹ hơn': 'Deeper thinking – slower, more careful',
  'Bỏ chọn': 'Clear',
  'CLI · tài khoản ChatGPT': 'CLI · ChatGPT account',
  'CLI · tài khoản Claude': 'CLI · Claude account',
  'Cho ngủ': 'Put to sleep',
  'Chèn vào ô soạn tin': 'Insert into the message box',
  'Chính tả': 'Spelling',
  'Chưa có hội thoại': 'No conversations yet',
  'Chưa có hội thoại nào được lưu.': 'No conversations saved yet.',
  'Chưa có key': 'No key',
  'Chưa có skill nào.': 'No skills yet.',
  'Chưa có tin nhắn': 'No messages yet',
  'Chưa có tài khoản': 'No accounts yet',
  'Chưa có tệp nào. Ảnh và tệp trong các hội thoại sẽ được lưu theo quy tắc trong Cài đặt.': 'No files yet. Photos and files in conversations are saved by the rules in Settings.',
  'Chỉ tắt thông báo của Mnemo; tin nhắn vẫn được lưu và vẫn đếm chưa đọc.': 'Only turns off Mnemo’s notifications; messages are still saved and counted as unread.',
  'Chọn các hội thoại': 'Choose conversations',
  'Chọn một hội thoại để xem tin nhắn đã lưu.': 'Choose a conversation to see its saved messages.',
  'Chọn result.json': 'Choose result.json',
  'Chọn thư mục kho tin nhắn': 'Choose the archive folder',
  'Chọn thư mục lưu tệp': 'Choose the downloads folder',
  'Chọn thư mục xuất': 'Choose the export folder',
  'Chọn thư mục đã giải nén': 'Choose the extracted folder',
  'Chọn tất cả': 'Select all',
  'Chọn tệp .zip': 'Choose the .zip file',
  'Chọn tệp .zip hoặc .txt': 'Choose the .zip or .txt file',
  'Chọn tệp skill (SKILL.md)': 'Choose a skill file (SKILL.md)',
  'Chọn ảnh đại diện': 'Choose a profile picture',
  'Chọn ảnh…': 'Choose picture…',
  'Chủ đề': 'Theme',
  'Claude Code (lệnh': 'Claude Code (the',
  'Codex CLI (lệnh': 'Codex CLI (',
  'Cài đặt': 'Settings',
  'Các trang web của ứng dụng thay đổi theo thời gian. Nếu Mnemo không lưu được tin của một ứng dụng, có thể chỉnh selector trong thư mục cấu hình dịch vụ (chép': 'The apps’ web pages change over time. If Mnemo stops saving an app’s messages, adjust its selectors in the service config folder (copy',
  'Có': 'Yes',
  'Cú pháp': 'Syntax',
  'Cấu hình dịch vụ': 'Service config',
  'Cần khóa này để mở kho trên máy khác. Hãy cất nó ở nơi an toàn (trình quản lý mật khẩu).': 'You need this key to open the archive on another computer. Keep it somewhere safe (a password manager).',
  'Dung lượng': 'Size',
  'Dung lượng tối đa mỗi tệp (MB)': 'Largest file (MB)',
  'Dán API key': 'Paste API key',
  'Dùng bản này': 'Use this',
  'Dùng chính tài khoản bạn đã đăng nhập trong': 'Uses the account you are signed in with in',
  'Dùng quy tắc riêng cho tài khoản này': 'Use separate rules for this account',
  'Dùng từ': 'Word choice',
  'Dùng để nhận ra tin của bạn khi nhập tệp xuất và khi AI gợi ý câu trả lời.': 'Used to recognise your own messages in imports and when the AI suggests replies.',
  'Dùng ảnh tự động': 'Use automatic picture',
  'Dấu câu': 'Punctuation',
  'Giao diện': 'Appearance',
  'Giữ bản đã lưu khi người gửi thu hồi': 'Keep the saved copy when the sender recalls it',
  'Giữ tin bị thu hồi': 'Keep recalled messages',
  'Gợi ý': 'Suggestion',
  'Gợi ý trả lời': 'Suggest replies',
  'Gửi': 'Send',
  'Hiện khóa': 'Show key',
  'Hiện nội dung tin nhắn': 'Show message text',
  'Hiện thông báo tin mới của tài khoản này': 'Show new-message notifications for this account',
  'Hiện tổng số của mọi tài khoản trên biểu tượng Mnemo': 'Show the total of all accounts on Mnemo’s icon',
  'Hiện ở góc phải dưới màn hình: ứng dụng, người gửi, người nhận, thời gian, nội dung. Bấm vào để mở đúng hội thoại.': 'Shown in the bottom-right corner: app, sender, recipient, time and text. Click to open the conversation.',
  'Hãy chọn ít nhất một hội thoại': 'Choose at least one conversation',
  'Hãy thêm tài khoản': 'Please add a',
  'trước.': 'account first.',
  'Hướng dẫn cho AI, vd: Viết lại cho lịch sự, xưng em, gọi anh/chị.': 'Instructions for the AI, e.g. Rewrite politely.',
  'Hướng dẫn chung cho AI': 'General instructions for the AI',
  'Hỏi về các hội thoại đã chọn… (Enter để gửi)': 'Ask about the selected conversations… (Enter to send)',
  'Hội thoại': 'Conversations',
  'Hội thoại loại trừ': 'Excluded conversations',
  'Hội thoại này': 'This conversation',
  'Hủy': 'Cancel',
  'Khi tắt, bấm nút M cạnh con trỏ hoặc Ctrl+Shift+Space để mở trợ lý viết.': 'When off, click the M button next to the cursor or press Ctrl+Shift+Space to open the writing assistant.',
  'Kho là một tệp mnemo.db mã hóa (SQLCipher). Khi đổi, Mnemo chép kho sang thư mục mới và giữ bản cũ.': 'The archive is one encrypted mnemo.db file (SQLCipher). When you change it, Mnemo copies the archive and keeps the old one.',
  'Kho tin nhắn': 'Archive',
  'Khoảng thời gian không hợp lệ': 'Invalid time range',
  'Khóa khôi phục': 'Recovery key',
  'Không bao giờ lưu đuôi tệp': 'Never save extensions',
  'Không lưu tin nhắn': 'Not saving messages',
  'Không thấy lỗi nào. 👍': 'No problems found. 👍',
  'Không tìm thấy': 'Nothing found',
  'Không xem trước được': 'No preview',
  'Khởi động & khay hệ thống': 'Startup & system tray',
  'Khởi động cùng Windows': 'Start with Windows',
  'Kèm tệp phương tiện': 'Include media',
  'Kết quả': 'Result',
  'Loại': 'Type',
  'Loại tệp': 'File types',
  'Luôn cho phép đuôi tệp': 'Always allow extensions',
  'Lúc lưu': 'Saved',
  'Lưu cả trong nhóm': 'Save in groups too',
  'Lưu key': 'Save key',
  'Lưu tin nhắn của tài khoản này': 'Save this account’s messages',
  'Lưu tin nhắn từ các ứng dụng': 'Save messages from the apps',
  'Lưu trữ': 'Storage',
  'Lưu tại': 'Saved in',
  'Lấp lịch sử cũ từ bản xuất chính thức của ứng dụng. Nhập lại nhiều lần không bị trùng.': 'Fill in older history from the apps’ official exports. Importing again never duplicates.',
  'Lọc hội thoại': 'Filter conversations',
  'Lỗi không rõ': 'Unknown error',
  'Messenger / Instagram (Tải thông tin của bạn)': 'Messenger / Instagram (Download your information)',
  'Mnemo chạy ẩn ở khay hệ thống (góc taskbar), không mở cửa sổ; tin nhắn vẫn được lưu và thông báo vẫn hiện.': 'Mnemo runs hidden in the system tray (taskbar corner) without opening a window; messages keep being saved and notifications still show.',
  'Mnemo tự lấy ảnh đại diện của bạn từ': 'Mnemo takes your profile picture from',
  'Mnemo – thông báo': 'Mnemo – notifications',
  'Mnemo – trợ lý viết': 'Mnemo – writing assistant',
  'Model (mặc định)': 'Model (default)',
  'Mã hóa': 'Encrypted',
  'Mã hóa tệp tự lưu': 'Encrypt saved files',
  'Mẫu…': 'Preset…',
  'Mặc định cho panel hội thoại': 'Default for the conversation panel',
  'Mặc định cho trợ lý viết': 'Default for the writing assistant',
  'Mặc định tắt: tin người khác đã thu hồi là quyền riêng tư của họ.': 'Off by default: a message someone recalled is their privacy.',
  'Mỗi dòng một tên hội thoại': 'One conversation name per line',
  'Mỗi tài khoản có hồ sơ trình duyệt riêng, nên bạn thêm được nhiều tài khoản của cùng một ứng dụng.': 'Each account has its own browser profile, so you can add several accounts of the same app.',
  'Mở': 'Open',
  'Mở Cài đặt để nhập khóa khôi phục hoặc chọn thư mục khác.': 'Open Settings to enter the recovery key or choose another folder.',
  'Mở kho bằng khóa này': 'Open the archive with this key',
  'Mở một hội thoại trong ứng dụng. Mnemo nhận ra hội thoại khi tin nhắn của nó đã được lưu.': 'Open a conversation in the app. Mnemo recognises it once its messages are saved.',
  'Mở một tài khoản hoặc chọn một hội thoại trong Kho tin nhắn.': 'Open an account or choose a conversation in the Archive.',
  'Mở một ứng dụng và vào hội thoại, hoặc nhập tệp xuất.': 'Open an app and go into a conversation, or import an export.',
  'Mở rộng thanh bên': 'Expand sidebar',
  'Mở thư mục': 'Open folder',
  'Nguồn': 'Source',
  'Ngữ pháp': 'Grammar',
  'Nhiều hội thoại': 'Several conversations',
  'Nhập': 'Imported',
  'Nhập khóa…': 'Enter key…',
  'Nhập tệp xuất': 'Import exports',
  'Nhập từ tệp .md': 'Import from .md file',
  'Nhập từ tệp xuất': 'Import from exports',
  'Nhập vào': 'Import into',
  'Nhật ký': 'Logs',
  'Nâng cao': 'Advanced',
  'Nút × chỉ ẩn cửa sổ. Thoát hẳn bằng chuột phải vào biểu tượng Mnemo ở khay → Thoát Mnemo.': 'The × button only hides the window. Quit by right-clicking the Mnemo tray icon → Quit Mnemo.',
  'Nội dung:': 'Content:',
  'Phiên này': 'This session',
  'Provider không trả về danh sách; hãy gõ tên model trực tiếp.': 'The provider returned no list; type the model name directly.',
  'Quy tắc lưu tệp riêng': 'Separate file rules',
  'Quy tắc tự động lưu tệp': 'Automatic file saving rules',
  'Sao chép': 'Copy',
  'Skill mới': 'New skill',
  'Skills viết lách': 'Writing skills',
  'Sáng': 'Light',
  'Sẽ lấy lại ảnh từ trang khi có': 'The picture will be taken from the page again when available',
  'Số tin chưa đọc trên taskbar': 'Unread count on the taskbar',
  'Số tin tối đa gửi cho AI': 'Most messages sent to the AI',
  'Sửa': 'Fix',
  'Sửa tất cả': 'Fix all',
  'Thay vào ô soạn tin': 'Replace the draft in the app',
  'Thoát Mnemo ngay': 'Quit Mnemo now',
  'Thu gọn thanh bên': 'Collapse sidebar',
  'Thêm': 'Add',
  'Thêm API khác (tương thích OpenAI)': 'Add another API (OpenAI-compatible)',
  'Thêm skill': 'Add skill',
  'Thêm tài khoản': 'Add account',
  'Thêm và đăng nhập': 'Add and sign in',
  'Thêm → Xuất đoạn chat': 'More → Export chat',
  'Thông báo': 'Notifications',
  'Thông báo của Mnemo': 'Mnemo notifications',
  'Thông báo theo tài khoản': 'Notifications per account',
  'Thông báo tin nhắn mới': 'New-message notifications',
  'Thư mục kho tin nhắn': 'Archive folder',
  'Thư mục tệp tải về': 'Downloads folder',
  'Thời gian hiển thị (giây)': 'Display time (seconds)',
  'Thử': 'Test',
  'Tin nhắn': 'Messages',
  'Tin nhắn và tệp đã lưu vẫn ở trong kho. Nếu chọn xóa dữ liệu đăng nhập, lần sau phải đăng nhập lại.': 'Saved messages and files stay in the archive. If you delete the sign-in data, you will have to sign in again.',
  'Tin nhắn đã bị thu hồi': 'Message recalled',
  'Trang của tài khoản': 'Account page',
  'Trung tâm tài khoản': 'Accounts Center',
  'Trên máy tính, mở': 'On your computer, open',
  'Trên điện thoại, mở hội thoại → ⋮ (hoặc tên hội thoại) →': 'On your phone, open the chat → ⋮ (or the chat name) →',
  'Trợ lý AI': 'AI assistant',
  'Trợ lý viết': 'Writing assistant',
  'Tuần này ai hẹn gặp tôi?': 'Who wants to meet me this week?',
  'Tài khoản': 'Accounts',
  'Tài liệu (pdf, docx, xlsx, zip…)': 'Documents (pdf, docx, xlsx, zip…)',
  'Tên': 'Name',
  'Tên của bạn trong': 'Your name in',
  'Tên hiển thị': 'Display name',
  'Tên hiển thị (để phân biệt nhiều tài khoản)': 'Display name (to tell several accounts apart)',
  'Tìm trong mọi tin nhắn (không cần dấu)': 'Search all messages (accents optional)',
  'Tóm tắt theo khoảng thời gian': 'Summarize a time range',
  'Tóm tắt tin chưa xem': 'Summarize unseen messages',
  'Tải danh sách model': 'Load model list',
  'Tải lại': 'Reload',
  'Tải thông tin của bạn': 'Download your information',
  'Tải tin cũ hơn': 'Load older messages',
  'Tải về': 'Downloaded',
  'Tất cả ứng dụng': 'All apps',
  'Tệp': 'Files',
  'Tệp mới sẽ lưu ở thư mục mới; tệp cũ giữ nguyên chỗ.': 'New files go to the new folder; existing files stay where they are.',
  'Tệp đã lưu': 'Saved files',
  'Tối': 'Dark',
  'Tổng hợp các khoản cần thanh toán': 'List the payments I owe',
  'Từ': 'From',
  'Đến': 'To',
  'Tự kiểm tra khi ngừng gõ': 'Check automatically when typing pauses',
  'Tự lưu': 'Auto-saved',
  'Tự tìm': 'Find',
  'Tự xóa tệp tự lưu sau (ngày)': 'Delete auto-saved files after (days)',
  'Tự động lưu tệp': 'Save files automatically',
  'Vào': 'Go to',
  'Văn phong': 'Style',
  'Xóa': 'Delete',
  'Xóa cuộc trò chuyện': 'Clear the conversation',
  'Xóa cả dữ liệu đăng nhập của tài khoản này': 'Also delete this account’s sign-in data',
  'Xóa key': 'Delete key',
  'Xóa provider': 'Remove provider',
  'Xóa tài khoản': 'Remove account',
  'Yêu cầu của bạn, vd: viết lại cho lịch sự, xưng em…': 'Your request, e.g. rewrite it more politely…',
  'Zalo, Discord, WeChat, Threads và Slack chưa có bản xuất chính thức mà Mnemo đọc được; tin nhắn của chúng được lưu khi bạn mở hội thoại trong Mnemo.': 'Zalo, Discord, WeChat, Threads and Slack have no official export Mnemo can read; their messages are saved when you open conversations in Mnemo.',
  'hoặc': 'or',
  'khi trang cho phép; nếu không, hãy chọn ảnh.': 'when the page allows it; otherwise choose a picture.',
  'llama.cpp (máy này)': 'llama.cpp (this computer)',
  'llama.cpp / máy chủ cục bộ': 'llama.cpp / local server',
  'mặc định': 'default',
  'nếu muốn có ảnh, rồi gửi tệp': 'if you want the photos, then send the',
  'rồi sửa), sau đó tải lại tài khoản. Dùng DevTools trong trang tài khoản để dò phần tử.': 'and edit it), then reload the account. Use DevTools on the account page to inspect elements.',
  'rồi đặt câu hỏi, vd:': 'then ask, e.g.:',
  'thành': 'to',
  'trên máy (hoặc nhập đường dẫn đầy đủ)': 'on this computer (or enter the full path)',
  'vd: Minh Nguyễn': 'e.g. Minh Nguyen',
  'vd: Tôi xưng': 'e.g. I call myself',
  "vd: Tôi xưng 'mình', thích văn phong ngắn gọn, thân thiện.": 'e.g. I like a short, friendly style.',
  'và tích Photos/Files nếu muốn lấy cả tệp. Lần xuất đầu tiên Telegram có thể bắt chờ 24 giờ.': 'and tick Photos/Files to include files. Telegram may make you wait 24 hours before the first export.',
  'về máy tính. Mỗi lần xuất là một hội thoại.': 'to your computer. Each export is one conversation.',
  'Âm thanh / tin nhắn thoại': 'Audio / voice messages',
  'Ô soạn tin đang trống.': 'The message box is empty.',
  'Ý muốn trả lời (không bắt buộc)': 'What you want to say (optional)',
  'Đang hỏi AI…': 'Asking the AI…',
  'Đang nhập…': 'Importing…',
  'Đang thử…': 'Testing…',
  'Đang tải…': 'Loading…',
  'Đang đọc tệp…': 'Reading the file…',
  'Đánh dấu đã xem': 'Mark as seen',
  'Đánh dấu đã xem tất cả': 'Mark all as seen',
  'Đã cho ngủ': 'Put to sleep',
  'Đã có key': 'Key saved',
  'Đã lưu key (mã hóa bằng Windows)': 'Key saved (encrypted by Windows)',
  'Đã mở kho': 'Archive opened',
  'Đã sao chép': 'Copied',
  'Đã tải lại': 'Reloaded',
  'Đóng': 'Close',
  'Đóng (Esc)': 'Close (Esc)',
  'Đóng trang để giải phóng bộ nhớ; mở lại khi bấm vào': 'Close the page to free memory; it reopens when you click it',
  'Định dạng khác': 'Other formats',
  'Đổi…': 'Change…',
  'Ảnh': 'Photos',
  'Ảnh đại diện': 'Profile picture',
  '•••••••• (đã lưu – nhập để thay)': '•••••••• (saved – type to replace)',
  '← Danh sách hội thoại': '← Conversations',
  '↑ Lên': '↑ Up',
  '→ Thông tin và quyền của bạn →': '→ Your information and permissions →',
  '→ chọn': '→ choose',
  '↓ Xuống': '↓ Down',
  '✓ Sửa lỗi': '✓ Proofread',
  'Trang chủ': 'Home',
  'Thanh bên Mnemo': 'Mnemo sidebar',
  'Bên trái': 'Left',
  'Bên phải': 'Right',
  'Đặt bên phải để thanh điều hướng riêng của ứng dụng (WhatsApp, Discord…) nằm sát mép trái cửa sổ, không bị kẹp giữa hai thanh.': 'Put it on the right so an app’s own navigation rail (WhatsApp, Discord…) sits at the window’s left edge instead of next to Mnemo’s.',
  'Ngôn ngữ': 'Language',
  'Theo Windows': 'Follow Windows',
  'Lịch sự hơn': 'More polite',
  'Thân thiện hơn': 'Friendlier',
  'Ngắn gọn hơn': 'Shorter',
  'Rõ ràng hơn': 'Clearer',
  'Trang trọng (công việc)': 'Formal (work)',
  'Dịch sang tiếng Anh': 'Translate to English',
  'Dịch sang tiếng Việt': 'Translate to Vietnamese',
  'Theo yêu cầu': 'As requested',
  'Tin nhắn mới': 'New message',
  'Chính tả, ngữ pháp, văn phong': 'Spelling, grammar, style',
  'Thêm ứng dụng': 'Add an app',
  'Ứng dụng': 'Apps',
  'Tổng quan': 'Overview',
  'Tin nhắn chưa đọc': 'Unread messages',
};

// Whole strings with numbers or names in them.
const I18N_PATTERNS = [
  [/^Tóm tắt tin chưa xem \((\d+)\)$/, 'Summarize unseen messages ($1)'],
  [/^<dịch vụ>(.*)$/, '<service>$1'],
  [/^Claude Code \(tài khoản Claude\)$/, 'Claude Code (Claude account)'],
  [/^ChatGPT \(Codex CLI\)$/, 'ChatGPT (Codex CLI)'],
  [/^Thêm (.+)$/, 'Add $1'],
  [/^Kho lưu trữ chưa mở: (.*)$/s, 'Archive not open: $1'],
  [/^Đã tải về: (.*)$/, 'Downloaded: $1'],
  [/^Không tìm thấy (.*)$/, 'Not found: $1'],
  [/^Xóa tài khoản "(.*)" khỏi Mnemo\?$/, 'Remove the account "$1" from Mnemo?'],
];

// Words inside composed strings ("230 tin · 4 chưa xem"), longest first.
const I18N_PHRASES = [
  ['Đang lưu tin nhắn', 'Saving messages'], ['tệp bỏ qua theo quy tắc', 'files skipped by rules'],
  ['(chuột phải: cài đặt)', '(right-click: settings)'], ['tin cập nhật', 'updated'], ['tin đã lưu', 'messages saved'],
  ['đã cắt bớt (tăng', 'shortened (raise'], ['trong Cài đặt)', 'in Settings)'], ['lỗi gần nhất', 'last error'],
  ['hội thoại', 'conversations'], ['chưa đọc', 'unread'], ['chưa xem', 'unseen'], ['phiên này', 'this session'],
  ['tin mới', 'new messages'], ['tệp lưu', 'files saved'], ['đã chọn', 'selected'], ['kết quả', 'results'],
  ['đã sửa', 'edited'], ['gợi ý', 'suggestions'], ['nhóm', 'group'], ['Xong', 'Done'], ['tin', 'messages'], ['tệp', 'files'],
];

function phraseSwap(s) {
  let out = s;
  for (const [vi, en] of I18N_PHRASES) {
    out = out.replace(new RegExp(`(?<![\\p{L}])${vi.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}(?![\\p{L}])`, 'gu'), en);
  }
  return out;
}

function tr(s) {
  if (window.LANG !== 'en' || s == null) return s;
  const raw = String(s);
  const t = raw.trim();
  if (!t) return raw;
  if (I18N[t] !== undefined) return raw.replace(t, I18N[t]);
  for (const [re, rep] of I18N_PATTERNS) {
    if (re.test(t)) return raw.replace(t, t.replace(re, rep));
  }
  if (/\d|·|—/.test(t)) return raw.replace(t, phraseSwap(t));
  return raw;
}

const I18N_ATTRS = ['title', 'placeholder', 'aria-label'];
let i18nBusy = false;

function translateTree(root) {
  if (window.LANG !== 'en' || !root) return;
  i18nBusy = true;
  const skip = (n) => {
    const e = n.nodeType === 1 ? n : n.parentElement;
    return !e || e.closest('[data-noi18n], script, style, textarea');
  };
  if (root.nodeType === 3) {
    if (!skip(root)) { const v = tr(root.nodeValue); if (v !== root.nodeValue) root.nodeValue = v; }
  } else if (root.nodeType === 1 || root.nodeType === 9 || root.nodeType === 11) {
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
    let n;
    while ((n = walker.nextNode())) {
      if (skip(n)) continue;
      const v = tr(n.nodeValue);
      if (v !== n.nodeValue) n.nodeValue = v;
    }
    const els = root.querySelectorAll ? root.querySelectorAll(I18N_ATTRS.map((a) => `[${a}]`).join(',')) : [];
    const list = root.nodeType === 1 ? [root, ...els] : els;
    list.forEach((e) => {
      if (e.closest('[data-noi18n]')) return;
      I18N_ATTRS.forEach((a) => {
        const v = e.getAttribute && e.getAttribute(a);
        if (v) { const t = tr(v); if (t !== v) e.setAttribute(a, t); }
      });
    });
  }
  i18nBusy = false;
}

const i18nObserver = new MutationObserver((records) => {
  if (i18nBusy || window.LANG !== 'en') return;
  for (const r of records) {
    if (r.type === 'characterData') translateTree(r.target);
    else if (r.type === 'attributes') translateTree(r.target);
    else r.addedNodes.forEach((n) => translateTree(n));
  }
  i18nObserver.takeRecords();
});

function setLang(lang) {
  window.LANG = lang === 'en' ? 'en' : 'vi';
  document.documentElement.lang = window.LANG;
  if (window.LANG === 'en') translateTree(document.body);
}

function locale() { return window.LANG === 'en' ? 'en-GB' : 'vi-VN'; }

(function watch() {
  const opts = { childList: true, subtree: true, characterData: true, attributes: true, attributeFilter: I18N_ATTRS };
  if (document.body) i18nObserver.observe(document.body, opts);
  else document.addEventListener('DOMContentLoaded', () => i18nObserver.observe(document.body, opts));
})();
