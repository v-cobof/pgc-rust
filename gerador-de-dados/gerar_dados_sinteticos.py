import pandas as pd
import numpy as np
import matplotlib.pyplot as plt
import seaborn as sns
from scipy import stats
from sklearn.neighbors import KernelDensity

# ============================================
# 1. CARREGAR OS DADOS ORIGINAIS
# ============================================
print("1. Carregando dados...")
df = pd.read_csv('entrada.csv')  # MUDE O NOME DO ARQUIVO AQUI
print(f"✅ Dados carregados: {len(df)} linhas")

# Visualizar primeiras linhas
print("\nPrimeiras 5 linhas:")
print(df.head())

# ============================================
# 2. ANALISAR DISTRIBUIÇÃO ORIGINAL
# ============================================
print("\n2. Analisando distribuição original...")

# Estatísticas descritivas
print("\nEstatísticas descritivas:")
print(df.describe())

# Verificar se os dados são normais (opcional)
for col in df.columns:
    stat, p_valor = stats.normaltest(df[col].dropna())
    print(f"{col}: p-valor do teste de normalidade = {p_valor:.6f}")
    if p_valor < 0.05:
        print(f"  → Não é normal (use KDE)")

# ============================================
# 3. TREINAR O MODELO KDE
# ============================================
print("\n3. Treinando modelo KDE...")

# Preparar dados (KDE espera numpy array 2D)
dados_original = df[['temperatura', 'umidade']].values

# Ajustar bandwidth - parâmetro importante!
# Menor = mais detalhes, maior = mais suave
bandwidth = 0.5  # Você pode ajustar este valor

kde = KernelDensity(bandwidth=bandwidth, kernel='gaussian')
kde.fit(dados_original)
print(f"✅ Modelo KDE treinado (bandwidth = {bandwidth})")

# ============================================
# 4. GERAR DADOS SINTÉTICOS
# ============================================
print("\n4. Gerando dados sintéticos...")
n_amostras = 40000  # Mesmo número de linhas do original

dados_sinteticos = kde.sample(n_samples=n_amostras)
df_sintetico = pd.DataFrame(dados_sinteticos, columns=['temperatura', 'umidade'])
print(f"✅ Geradas {n_amostras} linhas sintéticas")

# ============================================
# 5. TESTE DE KOLMOGOROV-SMIRNOV (KS Test)
# ============================================
print("\n5. Teste KS (Kolmogorov-Smirnov)...")
print("(Quanto maior o p-valor > 0.05, mais similares as distribuições)")

resultados_ks = {}
for col in df.columns:
    ks_stat, p_valor = stats.ks_2samp(df[col], df_sintetico[col])
    resultados_ks[col] = {'statistic': ks_stat, 'p-value': p_valor}
    
    print(f"\n{col}:")
    print(f"  KS statistic: {ks_stat:.4f}")
    print(f"  p-valor: {p_valor:.4f}")
    
    if p_valor > 0.05:
        print(f"  ✅ Distribuições similares (p > 0.05)")
    else:
        print(f"  ⚠️  Distribuições podem ser diferentes (p <= 0.05)")

# ============================================
# 6. COMPARAR CORRELAÇÕES
# ============================================
print("\n6. Comparando correlação entre variáveis...")

corr_original = df.corr().iloc[0,1]
corr_sintetico = df_sintetico.corr().iloc[0,1]

print(f"Correlação original: {corr_original:.4f}")
print(f"Correlação sintética: {corr_sintetico:.4f}")
print(f"Diferença: {abs(corr_original - corr_sintetico):.4f}")

# ============================================
# 7. GRÁFICOS DE COMPARAÇÃO
# ============================================
print("\n7. Gerando gráficos...")

# Configurar estilo dos gráficos
sns.set_style("whitegrid")
fig = plt.figure(figsize=(15, 10))

# Gráfico 1: Scatter plots lado a lado
ax1 = plt.subplot(2, 3, 1)
ax1.scatter(df['temperatura'], df['umidade'], alpha=0.3, s=1)
ax1.set_title('Original - Scatter Plot')
ax1.set_xlabel('Temperatura')
ax1.set_ylabel('Umidade')

ax2 = plt.subplot(2, 3, 2)
ax2.scatter(df_sintetico['temperatura'], df_sintetico['umidade'], alpha=0.3, s=1, c='orange')
ax2.set_title('Sintético (KDE) - Scatter Plot')
ax2.set_xlabel('Temperatura')
ax2.set_ylabel('Umidade')

# Gráfico 2: Histogramas (Temperatura)
ax3 = plt.subplot(2, 3, 3)
ax3.hist(df['temperatura'], bins=50, alpha=0.5, label='Original', density=True)
ax3.hist(df_sintetico['temperatura'], bins=50, alpha=0.5, label='Sintético', density=True)
ax3.set_title('Distribuição - Temperatura')
ax3.set_xlabel('Temperatura')
ax3.set_ylabel('Densidade')
ax3.legend()

# Gráfico 3: Histogramas (Umidade)
ax4 = plt.subplot(2, 3, 4)
ax4.hist(df['umidade'], bins=50, alpha=0.5, label='Original', density=True)
ax4.hist(df_sintetico['umidade'], bins=50, alpha=0.5, label='Sintético', density=True)
ax4.set_title('Distribuição - Umidade')
ax4.set_xlabel('Umidade')
ax4.set_ylabel('Densidade')
ax4.legend()

# Gráfico 4: Box plots
ax5 = plt.subplot(2, 3, 5)
dados_box = pd.DataFrame({
    'Original_Temp': df['temperatura'],
    'Sintético_Temp': df_sintetico['temperatura'],
    'Original_Umid': df['umidade'],
    'Sintético_Umid': df_sintetico['umidade']
})
dados_box.boxplot(ax=ax5)
ax5.set_title('Box Plot Comparison')
ax5.tick_params(axis='x', rotation=45)

# Gráfico 5: Densidade 2D (KDE dos dados)
ax6 = plt.subplot(2, 3, 6)
sns.kdeplot(data=df, x='temperatura', y='umidade', ax=ax6, label='Original')
sns.kdeplot(data=df_sintetico, x='temperatura', y='umidade', ax=ax6, label='Sintético', linestyle='--')
ax6.set_title('Densidade 2D (KDE)')
ax6.legend()

plt.tight_layout()
plt.savefig('comparacao_dados.png', dpi=150, bbox_inches='tight')
plt.show()

print("\n✅ Gráfico salvo como 'comparacao_dados.png'")

# ============================================
# 8. SALVAR DADOS SINTÉTICOS
# ============================================
print("\n8. Salvando dados sintéticos...")
df_sintetico.to_csv('dados_sinteticos.csv', index=False)
print("✅ Arquivo salvo como 'dados_sinteticos.csv'")

# ============================================
# 9. RESUMO FINAL
# ============================================
print("\n" + "="*50)
print("RESUMO FINAL")
print("="*50)
print(f"✅ Dados originais: {len(df)} linhas")
print(f"✅ Dados sintéticos: {len(df_sintetico)} linhas")
print(f"\nTeste KS (p-valor):")
for col, res in resultados_ks.items():
    print(f"  {col}: {res['p-value']:.4f}")
print(f"\nDiferença de correlação: {abs(corr_original - corr_sintetico):.4f}")
print("\n✨ Processo concluído com sucesso!")