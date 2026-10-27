"""Copy the selected submission images, check links, and package the draft."""
import json
from pathlib import Path
import re
import shutil
import zipfile

from PIL import Image, ImageDraw, ImageFont

HERE=Path(__file__).resolve().parent
VIDEO=Path('D:/personal/apps/ion-video')
ASSETS=[
    ('public/visual/work.png','01-workspace.png','Native project work'),
    ('public/visual/workspace.png','02-agent-teams.png','Agent team results'),
    ('public/visual/approval.png','03-sandbox-approval.png','Exact sandbox approval'),
    ('public/visual/result.png','04-sandbox-result.png','Sandbox result'),
    ('out/ion-demo-showcase-poster.png','05-artifact-showcase.png','Authored HTML showcase in recorded preview'),
    ('public/visual/phone.png','06-android.png','Android companion'),
]
folder=HERE/'screenshots'
folder.mkdir(exist_ok=True)
sheet=Image.new('RGB',(1600,1500),'#f6f7f5')
font=ImageFont.truetype('C:/Windows/Fonts/segoeui.ttf',22)
rows=[]
for i,(src,name,label) in enumerate(ASSETS):
    source=VIDEO/src
    target=folder/name
    shutil.copy2(source,target)
    with Image.open(target) as img:
        rows.append({'file':name,'source':str(source),'dimensions':list(img.size),'status':'Authored showcase composited into existing recording' if i==4 else 'Existing Ion capture; previously prepared crop'})
        thumb=img.convert('RGB')
        thumb.thumbnail((760,420))
        x=(i%2)*800+(800-thumb.width)//2
        y=(i//2)*500+48
        sheet.paste(thumb,(x,y))
        ImageDraw.Draw(sheet).text(((i%2)*800+30,(i//2)*500+12),f'{i+1}. {label}',font=font,fill='#121712')
sheet.save(HERE/'screenshots-contact.jpg',quality=90)
(HERE/'screenshots-sources.json').write_text(json.dumps(rows,indent=2),encoding='utf8')
doc=(HERE/'DEVPOST.md').read_text(encoding='utf8')
for ref in re.findall(r'!\[[^\]]*\]\(([^)]+)\)',doc):
    assert (HERE/ref).is_file(),ref
with zipfile.ZipFile(HERE/'ion-devpost-submission.zip','w',zipfile.ZIP_DEFLATED) as archive:
    for name in ['DEVPOST.md','screenshots-sources.json','screenshots-contact.jpg']:
        archive.write(HERE/name,name)
    notes=(HERE/'SUBMISSION-NOTES.md').read_text(encoding='utf8').replace('../android-release-1.0.1.md','android-release-1.0.1.md')
    archive.writestr('SUBMISSION-NOTES.md',notes)
    archive.write(HERE.parent/'android-release-1.0.1.md','android-release-1.0.1.md')
    for _,name,_ in ASSETS:archive.write(folder/name,f'screenshots/{name}')
print('Prepared 6 screenshots; all Markdown image links resolve; ZIP ready.')
