import os
import pandas as pd
import numpy as np
import matplotlib.pyplot as plt
import seaborn as sns

# ==============================================================================
# CONFIGURAÇÕES DO MODELO DE TREINAMENTO DO TOMATEIRO
# ==============================================================================
# Define os limites físicos de temperatura (C) e umidade (%) para cada uma das
# 11 classes de status agrometeorológico do tomateiro, estabelecidos no artigo de base.
training_dataset = [
    {"category": 0, "name": "Critical dry", "t_min": 0.0, "t_max": 4.0, "h_min": 0.0, "h_max": 35.0},
    {"category": 1, "name": "Lower fail", "t_min": 0.0, "t_max": 11.0, "h_min": 40.0, "h_max": 65.0},
    {"category": 2, "name": "Marginal", "t_min": 12.0, "t_max": 28.0, "h_min": 40.0, "h_max": 55.0},
    {"category": 3, "name": "Upper Fail", "t_min": 29.0, "t_max": 47.0, "h_min": 40.0, "h_max": 55.0},
    {"category": 4, "name": "Cold and Humid", "t_min": 0.0, "t_max": 11.0, "h_min": 70.0, "h_max": 100.0},
    {"category": 5, "name": "Lower optimal", "t_min": 12.0, "t_max": 14.0, "h_min": 60.0, "h_max": 100.0},
    {"category": 6, "name": "Optimal", "t_min": 15.0, "t_max": 17.0, "h_min": 60.0, "h_max": 100.0},
    {"category": 7, "name": "Upper Optimal", "t_min": 18.0, "t_max": 27.0, "h_min": 60.0, "h_max": 100.0},
    {"category": 8, "name": "Upper Marginal", "t_min": 28.0, "t_max": 31.0, "h_min": 60.0, "h_max": 100.0},
    {"category": 9, "name": "Upper Fail (high hum)", "t_min": 32.0, "t_max": 35.0, "h_min": 60.0, "h_max": 100.0},
    {"category": 10, "name": "Critical", "t_min": 36.0, "t_max": 47.0, "h_min": 60.0, "h_max": 100.0},
]

# Normaliza uma variável física no intervalo [0, 1] com base nos limites min e max globais
def normalize_value(value, min_val, max_val):
    return (value - min_val) / (max_val - min_val)

# Classifica uma leitura contínua (temp, hum) usando o algoritmo k-vizinhos mais próximos (kNN)
def knn_classify(temp, hum, k=3):
    # Normaliza a entrada com base nos limites globais dos sensores (temp: [0, 47], hum: [0, 100])
    norm_temp = normalize_value(temp, 0.0, 47.0)
    norm_hum = normalize_value(hum, 0.0, 100.0)
    
    # Calcula a distância euclidiana para o ponto médio normalizado de cada classe de treino
    distances = []
    for sample in training_dataset:
        sample_temp_mid = (sample["t_min"] + sample["t_max"]) / 2.0
        sample_hum_mid = (sample["h_min"] + sample["h_max"]) / 2.0
        
        norm_sample_temp = normalize_value(sample_temp_mid, 0.0, 47.0)
        norm_sample_hum = normalize_value(sample_hum_mid, 0.0, 100.0)
        
        dist = np.sqrt((norm_temp - norm_sample_temp)**2 + (norm_hum - norm_sample_hum)**2)
        distances.append((dist, sample["category"], sample["name"]))
        
    # Ordena as distâncias e seleciona os k vizinhos mais próximos
    distances.sort(key=lambda x: x[0])
    neighbors = distances[:k]
    
    # Executa a votação majoritária entre os vizinhos selecionados
    votes = {}
    for dist, cat, name in neighbors:
        if cat not in votes:
            votes[cat] = [0, name]
        votes[cat][0] += 1
        
    max_votes = -1
    best_cat = None
    best_name = None
    for cat, val in votes.items():
        if val[0] > max_votes:
            max_votes = val[0]
            best_cat = cat
            best_name = val[1]
            
    return best_name

# Função auxiliar para resolver caminhos dos arquivos de dados
def find_file(filename):
    if os.path.exists(filename):
        return filename
    parent_path = os.path.join("..", filename)
    if os.path.exists(parent_path):
        return parent_path
    sub_path = os.path.join("gerador-de-dados", filename)
    if os.path.exists(sub_path):
        return sub_path
    exp_path = os.path.join("analise-experimentos", "experimento-1", filename)
    if os.path.exists(exp_path):
        return exp_path
    raise FileNotFoundError(f"Não foi possível encontrar o arquivo {filename}")

# Tenta resolver caminhos e carregar os dados reais e sintéticos
try:
    csv_original = find_file("entrada.csv")
    csv_sintetico = find_file("dados_sinteticos_gerados_rust.csv")
except FileNotFoundError as e:
    print(f"Erro: {e}")
    exit(1)

# Carrega os conjuntos de dados em DataFrames do Pandas
print(f"[*] Lendo dados originais de: {csv_original}")
df_orig = pd.read_csv(csv_original)
print(f"[*] Lendo dados sintéticos de: {csv_sintetico}")
df_sint = pd.read_csv(csv_sintetico)

# Realiza a partição dos primeiros 720 registros para simular os turnos (Rounds) do artigo:
# R1: primeiros 360 pontos (período da madrugada/estável)
# R2: seguintes 360 pontos (período da manhã/transição térmica)
r1 = df_orig.iloc[:361].copy()
r2 = df_orig.iloc[361:721].copy()

# Classifica os registros de cada turno via kNN
print("Classificando dados via kNN (k=3)...")
r1['Category'] = r1.apply(lambda row: knn_classify(row['temperatura'], row['umidade']), axis=1)
r2['Category'] = r2.apply(lambda row: knn_classify(row['temperatura'], row['umidade']), axis=1)

# Amostra subconjuntos menores de 2.000 pontos para simplificar a renderização dos pairplots globais
df_orig_sample = df_orig.sample(n=2000, random_state=42).copy()
df_sint_sample = df_sint.sample(n=2000, random_state=42).copy()

df_orig_sample['Category'] = df_orig_sample.apply(lambda row: knn_classify(row['temperatura'], row['umidade']), axis=1)
df_sint_sample['Category'] = df_sint_sample.apply(lambda row: knn_classify(row['temperatura'], row['umidade']), axis=1)

out_dir = 'analise-experimentos'
if not os.path.exists(out_dir):
    os.makedirs(out_dir)

# ==============================================================================
# FIGURA 3 ORIGINAL: Curvas de contorno de densidade de probabilidade conjunta
# ==============================================================================
# Gera o gráfico de estimativa de densidade de kernel bidimensional (KDE) 
# dividindo os dados reais entre o Round 1 e o Round 2.
print("Gerando Figura 3 - Original (Contour Curve Plot)...")
sns.set_style("whitegrid")
fig, axes = plt.subplots(1, 2, figsize=(12, 5))
sns.kdeplot(data=r1, x='temperatura', y='umidade', fill=True, ax=axes[0], cmap='Blues', thresh=0.05)
axes[0].set_title('Round 1 (00:00 - 06:00) - Original', fontsize=12, fontweight='bold')
axes[0].set_xlabel('Temperatura (C)')
axes[0].set_ylabel('Umidade (%)')
axes[0].set_xlim(12, 32)
axes[0].set_ylim(20, 85)

sns.kdeplot(data=r2, x='temperatura', y='umidade', fill=True, ax=axes[1], cmap='Blues', thresh=0.05)
axes[1].set_title('Round 2 (06:01 - 12:00) - Original', fontsize=12, fontweight='bold')
axes[1].set_xlabel('Temperatura (C)')
axes[1].set_ylabel('Umidade (%)')
axes[1].set_xlim(12, 32)
axes[1].set_ylim(20, 85)

plt.tight_layout()
fig_path_3_orig = os.path.join(out_dir, 'densidade_artigo_original.png')
plt.savefig(fig_path_3_orig, dpi=150)
plt.close()

# ==============================================================================
# FIGURA 3 SIMULADA: Curvas de contorno para a base gerada em Rust
# ==============================================================================
# Amostra dois blocos independentes para mostrar que o KDE estocástico estático
# gera dados com a densidade populacional completa em cada amostra aleatória.
print("Gerando Figura 3 - Sintetico (Contour Curve Plot)...")
fig, axes = plt.subplots(1, 2, figsize=(12, 5))
r1_sint = df_sint.sample(n=361, random_state=42)
r2_sint = df_sint.sample(n=360, random_state=24)

sns.kdeplot(data=r1_sint, x='temperatura', y='umidade', fill=True, ax=axes[0], cmap='Oranges', thresh=0.05)
axes[0].set_title('Amostra Sintetica 1 (Equiv. R1)', fontsize=12, fontweight='bold')
axes[0].set_xlabel('Temperatura (C)')
axes[0].set_ylabel('Umidade (%)')
axes[0].set_xlim(12, 32)
axes[0].set_ylim(20, 85)

sns.kdeplot(data=r2_sint, x='temperatura', y='umidade', fill=True, ax=axes[1], cmap='Oranges', thresh=0.05)
axes[1].set_title('Amostra Sintetica 2 (Equiv. R2)', fontsize=12, fontweight='bold')
axes[1].set_xlabel('Temperatura (C)')
axes[1].set_ylabel('Umidade (%)')
axes[1].set_xlim(12, 32)
axes[1].set_ylim(20, 85)

plt.tight_layout()
fig_path_3_sint = os.path.join(out_dir, 'densidade_artigo_sintetico.png')
plt.savefig(fig_path_3_sint, dpi=150)
plt.close()

# ==============================================================================
# FIGURA 4 ORIGINAL: Correlograma das categorias agronômicas por turno
# ==============================================================================
# Plota a relação de dispersão colorida pelas categorias agronômicas do tomateiro
# geradas pelo kNN, com histogramas marginais nas diagonais.
print("Gerando Figura 4 - Original (Correlograms)...")
r1_eng = r1.rename(columns={'temperatura': 'Temperature', 'umidade': 'Humidity'})
r2_eng = r2.rename(columns={'temperatura': 'Temperature', 'umidade': 'Humidity'})

g1 = sns.pairplot(r1_eng, vars=['Temperature', 'Humidity'], hue='Category', palette='viridis', diag_kind='kde', height=3.5)
g1.fig.suptitle('ROUND 1 - Correlograma de Categorias (Original)', y=1.02, fontsize=12, fontweight='bold')
fig_path_4_r1 = os.path.join(out_dir, 'correlograma_r1_original.png')
g1.savefig(fig_path_4_r1, dpi=150)
plt.close()

g2 = sns.pairplot(r2_eng, vars=['Temperature', 'Humidity'], hue='Category', palette='tab10', diag_kind='kde', height=3.5)
g2.fig.suptitle('ROUND 2 - Correlograma de Categorias (Original)', y=1.02, fontsize=12, fontweight='bold')
fig_path_4_r2 = os.path.join(out_dir, 'correlograma_r2_original.png')
g2.savefig(fig_path_4_r2, dpi=150)
plt.close()

# ==============================================================================
# FIGURA 4 SIMULADA: Correlograma das categorias para os dados sintéticos
# ==============================================================================
print("Gerando Figura 4 - Sintetico (Correlograms)...")
r1_sint_eng = r1_sint.rename(columns={'temperatura': 'Temperature', 'umidade': 'Humidity'})
r2_sint_eng = r2_sint.rename(columns={'temperatura': 'Temperature', 'umidade': 'Humidity'})
r1_sint_eng['Category'] = r1_sint_eng.apply(lambda row: knn_classify(row['Temperature'], row['Humidity']), axis=1)
r2_sint_eng['Category'] = r2_sint_eng.apply(lambda row: knn_classify(row['Temperature'], row['Humidity']), axis=1)

g1_sint = sns.pairplot(r1_sint_eng, vars=['Temperature', 'Humidity'], hue='Category', palette='viridis', diag_kind='kde', height=3.5)
g1_sint.fig.suptitle('Amostra Sintetica 1 - Correlograma (Sintetico)', y=1.02, fontsize=12, fontweight='bold')
fig_path_4_r1_sint = os.path.join(out_dir, 'correlograma_r1_sintetico.png')
g1_sint.savefig(fig_path_4_r1_sint, dpi=150)
plt.close()

g2_sint = sns.pairplot(r2_sint_eng, vars=['Temperature', 'Humidity'], hue='Category', palette='tab10', diag_kind='kde', height=3.5)
g2_sint.fig.suptitle('Amostra Sintetica 2 - Correlograma (Sintetico)', y=1.02, fontsize=12, fontweight='bold')
fig_path_4_r2_sint = os.path.join(out_dir, 'correlograma_r2_sintetico.png')
g2_sint.savefig(fig_path_4_r2_sint, dpi=150)
plt.close()

# ==============================================================================
# COMPARAÇÃO GERAL GLOBAL: Densidade de contorno bidimensional para toda a base
# ==============================================================================
# Plota lado a lado a distribuição conjunta global da base real e da base gerada.
print("Gerando Comparacao de Contorno Global (Original vs Sintetico)...")
df_orig_global = df_orig.sample(n=min(5000, len(df_orig)), random_state=42) if len(df_orig) > 5000 else df_orig
df_sint_global = df_sint.sample(n=min(5000, len(df_sint)), random_state=42) if len(df_sint) > 5000 else df_sint

fig, axes = plt.subplots(1, 2, figsize=(12, 5))
sns.kdeplot(data=df_orig_global, x='temperatura', y='umidade', fill=True, ax=axes[0], cmap='Blues', thresh=0.05)
axes[0].set_title('Base de Teste Completa - Original (Amostra)', fontsize=12, fontweight='bold')
axes[0].set_xlabel('Temperatura (C)')
axes[0].set_ylabel('Umidade (%)')
axes[0].set_xlim(5, 35)
axes[0].set_ylim(10, 95)

sns.kdeplot(data=df_sint_global, x='temperatura', y='umidade', fill=True, ax=axes[1], cmap='Oranges', thresh=0.05)
axes[1].set_title('Base Sintetica Completa - Rust KDE (Amostra)', fontsize=12, fontweight='bold')
axes[1].set_xlabel('Temperatura (C)')
axes[1].set_ylabel('Umidade (%)')
axes[1].set_xlim(5, 35)
axes[1].set_ylim(10, 95)

plt.tight_layout()
fig_path_global = os.path.join(out_dir, 'densidade_global_comparacao.png')
plt.savefig(fig_path_global, dpi=150)
plt.close()

print(f"[OK] Graficos salvos com sucesso na pasta {out_dir}!")
