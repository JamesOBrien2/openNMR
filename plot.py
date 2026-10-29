from pathlib import Path
import csv, math
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt

root = Path(__file__).resolve().parent
with (root / 'measurements.csv').open() as f:
    rows = list(csv.DictReader(f))
labels = [row['Spectrum'] for row in rows]
previous = [float(row['Previous RMS percent']) for row in rows]
current = [float(row['Updated RMS percent']) for row in rows]
assert rows and all(math.isfinite(x) and x > 0 for x in previous + current)

plt.rcParams.update({'font.family':'DejaVu Sans','font.size':11,'axes.spines.top':False,'axes.spines.right':False})
fig,ax=plt.subplots(figsize=(10.6,5.6),layout='constrained')
for i,(a,b) in enumerate(zip(previous,current)):
    ax.plot([a,b],[i,i],color='#b8c1cb',lw=2,zorder=1)
ax.scatter(previous,range(4),color='#a95525',s=75,label='Previous version',zorder=3)
ax.scatter(current,range(4),color='#1b66a5',s=75,label='Updated version',zorder=3)
for i,(a,b) in enumerate(zip(previous,current)):
    ax.annotate(f'{a:.3f}%',(a,i),xytext=(0,12),textcoords='offset points',ha='center',color='#91491e',fontsize=10)
    ax.annotate(f'{b:.3f}%',(b,i),xytext=(0,-20),textcoords='offset points',ha='center',color='#165486',fontsize=10)
ax.set_xscale('log'); ax.set_xlim(.02,60)
ax.set_xticks([.02,.05,.1,.5,1,5,10,50],labels=['0.02','0.05','0.1','0.5','1','5','10','50'])
ax.set_yticks(range(4),labels=labels); ax.set_ylim(3.55,-.65)
ax.set_xlabel('Relative RMS difference (%) — logarithmic scale; lower is better',labelpad=13)
ax.grid(axis='x',color='#e0e5ea'); ax.set_axisbelow(True)
ax.legend(loc='upper right',frameon=False,ncol=2,bbox_to_anchor=(1,1.18))
fig.suptitle('Automatic phase correction: consistency',fontsize=18,fontweight='bold',ha='left',x=.02)
fig.supxlabel('Same raw data; worst result across four known added phase errors.\nConsistency does not establish absolute accuracy. The broad single-signal case remains unreliable.',fontsize=10,color='#4b5563',ha='left',x=.02)
fig.savefig(root / 'phase-consistency.png',dpi=160,facecolor='white')
print('Created phase-consistency.png')
